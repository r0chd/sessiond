mod session_management_v1 {
    hyprwire::include_protocol!("session_management_v1");
    pub use server::*;
}
mod session_core_v1 {
    hyprwire::include_protocol!("session_core_v1");
    pub use server::*;
    pub use spec::*;
}
mod seat_v1 {
    hyprwire::include_protocol!("seat_v1");
    pub use server::*;
    pub use spec::*;
}

mod cgroup;
#[cfg(feature = "consolekit")]
mod consolekit;
mod device;
mod ecs;
mod seat;
mod session;
mod terminal;
mod user;

#[cfg(test)]
mod session_management_v1_client {
    hyprwire::include_protocol!("session_management_v1");
    pub use client::*;
}
#[cfg(test)]
mod session_core_v1_client {
    hyprwire::include_protocol!("session_core_v1");
    pub use client::*;
}

#[cfg(test)]
mod tests;

use bevy_ecs::{entity, world};
use calloop::{generic, signals};
use clap::Parser;
use hyprwire::server;
use rustix::{net, process, system};
use session_core_v1::{session_core_manager, session_core_session, session_core_user};
use session_management_v1::session_manager;
use std::os::fd::AsRawFd;
use std::os::unix::fs::PermissionsExt;
use std::{env, fs, path, time};

#[derive(Parser)]
struct Args {
    #[arg(
        long,
        value_name = "USER",
        default_value = "root",
        help = "Allow USER to bind seat_v1"
    )]
    seat_user: String,
    #[arg(
        long,
        value_name = "GROUP",
        default_value = "root",
        help = "Allow members of GROUP to bind seat_v1"
    )]
    seat_group: String,
}

struct SeatAccess {
    user: process::Uid,
    group: process::Gid,
}

impl SeatAccess {
    const fn new(user: process::Uid, group: process::Gid) -> Self {
        Self { user, group }
    }

    fn allows(&self, creds: &net::UCred) -> bool {
        if self.user == creds.uid || self.group == creds.gid {
            return true;
        }

        Self::process_groups(creds.pid.as_raw_pid().cast_unsigned())
            .is_ok_and(|groups| groups.contains(&self.group))
    }

    fn process_groups(pid: u32) -> anyhow::Result<Vec<process::Gid>> {
        let status = fs::read_to_string(format!("/proc/{pid}/status"))?;
        let Some(groups) = status.lines().find_map(|line| line.strip_prefix("Groups:")) else {
            return Ok(Vec::new());
        };

        groups
            .split_whitespace()
            .map(|group| {
                group
                    .parse::<process::RawGid>()
                    .map(process::Gid::from_raw)
                    .map_err(Into::into)
            })
            .collect()
    }
}

struct Sessiond {
    next_session_id: session::SessionId,
    info_managers: Vec<session_core_manager::SessionCoreManager>,
    cgroup_manager: cgroup::CgroupManager,
    session_entities: Vec<entity::Entity>,
    user_entities: Vec<entity::Entity>,
    seat_entities: Vec<entity::Entity>,
    client_entities: Vec<entity::Entity>,
    seat_access: SeatAccess,
    #[cfg(feature = "consolekit")]
    consolekit: consolekit::Server,
    shutting_down: bool,
    loop_signal: calloop::LoopSignal,
    world: world::World,
}

impl Sessiond {
    fn remove_session(&mut self, session_id: session::SessionId) -> Option<entity::Entity> {
        let idx = self.session_entities.iter().position(|entity| {
            self.world
                .get::<ecs::Session>(*entity)
                .is_some_and(|session| session.0 == session_id)
        })?;
        let entity = self.session_entities.remove(idx);
        self.world.despawn(entity);
        #[cfg(feature = "consolekit")]
        self.consolekit.remove_session(session_id);
        Some(entity)
    }

    #[cfg(feature = "consolekit")]
    fn sync_consolekit_session(&self, session_id: session::SessionId, entity: entity::Entity) {
        if let Some(info) = consolekit::SessionInfo::from_world(&self.world, entity, session_id) {
            self.consolekit.upsert_session(info);
        }
    }
}

impl session_management_v1::SessionManagementV1Handler for Sessiond {}

impl session_core_v1::SessionCoreV1Handler for Sessiond {
    fn bind(&mut self, object: session_core_manager::SessionCoreManager) {
        session::sync_vt_session_states(
            &mut self.world,
            &self.seat_entities,
            &self.session_entities,
            &self.user_entities,
        );

        session::entities(&self.world, &self.session_entities)
            .filter_map(|entity| session::id(&self.world, entity))
            .for_each(|session_id| {
                object.send_session_added(session_id.as_raw());
            });
        object.send_done();
        self.info_managers.push(object);
    }
}

impl hyprwire::Dispatch<session_manager::SessionManager> for Sessiond {
    fn event(
        &mut self,
        object: &session_manager::SessionManager,
        event: <session_manager::SessionManager as hyprwire::Object>::Event<'_>,
    ) {
        let Some(client) = object.client() else {
            return;
        };

        #[cfg(not(test))]
        if !client.creds().uid.is_root() {
            log::warn!(uid = client.creds().uid.as_raw(); "rejected request from non-root client");
            return;
        }

        match event {
            session_manager::Event::CreateSession {
                uid,
                vt,
                is_local,
                remote_host,
            } => {
                let session_id = self.next_session_id;
                self.next_session_id = self.next_session_id.next();
                let is_local = is_local != 0;
                let username = users::get_user_by_uid(uid)
                    .map(|user| user.name().to_string_lossy().into_owned())
                    .unwrap_or_default();
                log::info!(session_id = session_id.as_raw(), uid, username = username.as_str(), vt, is_local, remote_host = remote_host.as_str(); "session created");

                let leader_id = client.creds().pid.as_raw_pid();

                if let Err(e) = self
                    .cgroup_manager
                    .create_session_cgroup(uid, session_id, leader_id)
                {
                    log::error!(session_id = session_id.as_raw(), error = e.to_string().as_str(); "failed to create session cgroup");
                }

                seat::refresh_current_vts(&mut self.world, &self.seat_entities);

                let mut session_seat_entity = if vt != 0 {
                    self.seat_entities.first().copied()
                } else {
                    None
                };
                let mut session_seat = session_seat_entity.and_then(|seat_entity| {
                    self.world
                        .get::<ecs::SeatName>(seat_entity)
                        .map(|seat| seat.0.clone())
                        .filter(|seat| !seat.is_empty())
                });
                let mut active_state = session_core_v1::ActiveState::Online;

                user::create_runtime_dir(uid);

                if let Some((seat_entity, seat)) =
                    self.seat_entities.iter().copied().find_map(|seat_entity| {
                        let mode = self.world.get::<ecs::SeatMode>(seat_entity)?;
                        if mode.0 != ecs::SeatKind::VtBound {
                            return None;
                        }
                        let current_vt = self.world.get::<ecs::CurrentVt>(seat_entity)?.0;
                        (current_vt == vt).then(|| {
                            self.world
                                .get::<ecs::SeatName>(seat_entity)
                                .map(|seat| (seat_entity, seat.0.clone()))
                        })?
                    })
                {
                    session_seat_entity = Some(seat_entity);
                    session_seat = Some(seat);
                    active_state = session_core_v1::ActiveState::Active;
                }

                let session_entity = ecs::insert_session(
                    &mut self.world,
                    session_id,
                    uid,
                    username,
                    vt,
                    session_seat.clone(),
                    session_seat_entity,
                    time::SystemTime::now(),
                    leader_id,
                    is_local,
                    remote_host,
                    Vec::new(),
                    active_state,
                );
                self.session_entities.push(session_entity);
                #[cfg(feature = "consolekit")]
                self.sync_consolekit_session(session_id, session_entity);
                user::sync_all(&mut self.world, &self.user_entities, &self.session_entities);

                object.send_session_created(
                    session_id.as_raw(),
                    session_seat.as_deref().unwrap_or_default(),
                );

                for manager in &self.info_managers {
                    manager.send_session_added(session_id.as_raw());
                }
            }
            session_manager::Event::DestroySession { session_id } => {
                let session_id = session::SessionId::from_raw(session_id);
                let Some(entity) = session::entity(&self.world, &self.session_entities, session_id)
                else {
                    log::warn!(session_id = session_id.as_raw(); "destroy requested for unknown session");
                    return;
                };

                let uid = self.world.get::<ecs::Uid>(entity).map_or(0, |uid| uid.0);
                let username = self
                    .world
                    .get::<ecs::Username>(entity)
                    .map_or("", |username| username.0.as_str());
                log::info!(session_id = session_id.as_raw(), uid, username; "session destroyed");

                if let Some(mut active_state) = self.world.get_mut::<ecs::ActiveState>(entity) {
                    active_state.0 = session_core_v1::ActiveState::Closing;
                }
                #[cfg(feature = "consolekit")]
                self.sync_consolekit_session(session_id, entity);
                if let Some(objects) = self.world.get::<ecs::SessionObjects>(entity) {
                    for object in &objects.0 {
                        object.send_active(session_core_v1::ActiveState::Closing);
                    }
                }
                if let Some(objects) = self.world.get::<ecs::SessionObjects>(entity) {
                    for object in &objects.0 {
                        object.send_destroyed();
                    }
                }
                let _ = self.remove_session(session_id);
                user::sync_all(&mut self.world, &self.user_entities, &self.session_entities);

                for manager in &self.info_managers {
                    manager.send_session_removed(session_id.as_raw());
                }

                if let Err(e) = self.cgroup_manager.poll_destroy(session_id) {
                    log::error!(session_id = session_id.as_raw(), error = e.to_string().as_str(); "failed to poll destroy session cgroup");
                }

                if !user::is_lingering(&self.world, &self.user_entities, uid)
                    && session::entities_by_uid(&self.world, &self.session_entities, uid)
                        .next()
                        .is_none()
                    && let Err(e) = user::remove_runtime_dir(uid)
                {
                    log::error!(session_id = session_id.as_raw(), uid, error = e.to_string().as_str(); "failed to remove runtime dir");
                }
            }
            session_manager::Event::Destroy => {}
        }
    }
}

impl hyprwire::Dispatch<session_core_manager::SessionCoreManager> for Sessiond {
    fn event(
        &mut self,
        object: &session_core_manager::SessionCoreManager,
        event: <session_core_manager::SessionCoreManager as hyprwire::Object>::Event<'_>,
    ) {
        match event {
            session_core_manager::Event::GetSession {
                session_core_session,
                session_id,
            } => {
                session::sync_vt_session_states(
                    &mut self.world,
                    &self.seat_entities,
                    &self.session_entities,
                    &self.user_entities,
                );

                let session_id = session::SessionId::from_raw(session_id);
                if let Some(entity) =
                    session::entity(&self.world, &self.session_entities, session_id)
                {
                    session::add_session_object(
                        &mut self.world,
                        session_id,
                        entity,
                        session_core_session,
                    );
                    return;
                }

                log::warn!(session_id = session_id.as_raw(); "get_session requested for unknown session");
                session_core_session.send_destroyed();
            }
            session_core_manager::Event::GetSessionByPid {
                session_core_session,
                pid,
            } => {
                session::sync_vt_session_states(
                    &mut self.world,
                    &self.seat_entities,
                    &self.session_entities,
                    &self.user_entities,
                );

                let Ok(Some(session_id)) = self.cgroup_manager.find_session_by_pid(pid) else {
                    log::warn!(pid; "get_session_by_pid: no session found for pid");
                    session_core_session.send_destroyed();
                    return;
                };

                if let Some(entity) =
                    session::entity(&self.world, &self.session_entities, session_id)
                {
                    session::add_session_object(
                        &mut self.world,
                        session_id,
                        entity,
                        session_core_session,
                    );
                    return;
                }

                log::warn!(session_id = session_id.as_raw(); "get_session_by_pid requested for unknown session");
                session_core_session.send_destroyed();
            }
            session_core_manager::Event::GetSessionByUid {
                session_core_session,
                uid,
            } => {
                session::sync_vt_session_states(
                    &mut self.world,
                    &self.seat_entities,
                    &self.session_entities,
                    &self.user_entities,
                );

                if let Some(entity) =
                    session::entity_by_uid(&self.world, &self.session_entities, uid)
                    && let Some(session_id) = session::id(&self.world, entity)
                {
                    session::add_session_object(
                        &mut self.world,
                        session_id,
                        entity,
                        session_core_session,
                    );
                    return;
                }

                log::warn!(uid; "get_session_by_uid requested for unknown session");
                session_core_session.send_destroyed();
            }
            session_core_manager::Event::GetUser {
                session_core_user,
                uid,
            } => {
                let Some(entity) = user::get_or_create(
                    &mut self.world,
                    &mut self.user_entities,
                    &self.session_entities,
                    uid,
                ) else {
                    log::warn!(uid; "get_user requested for unknown user");
                    session_core_user.send_not_found();
                    return;
                };

                user::add_user_object(&mut self.world, entity, session_core_user);
            }
            session_core_manager::Event::Reboot => {
                let Some(client) = object.client() else {
                    return;
                };

                // let user reboot only if there are no other user sessions active
                let foreign_session_count = session::entities(&self.world, &self.session_entities)
                    .filter_map(|entity| self.world.get::<ecs::Uid>(entity))
                    .filter(|session| session.0 != client.creds().uid.as_raw())
                    .count();

                if !client.creds().uid.is_root() && foreign_session_count > 0 {
                    log::warn!(uid = client.creds().uid.as_raw(), pid = client.creds().pid.as_raw_pid(), foreign_session_count; "reboot denied: multiple sessions active");
                    object.send_error(
                        session_core_v1::ErrorCode::ActiveSessions,
                        "cannot reboot while multiple sessions are active (root privileges required)",
                    );
                    return;
                }

                log::info!(uid = client.creds().uid.as_raw(), pid = client.creds().pid.as_raw_pid(), foreign_session_count; "reboot accepted");

                for session_id in session::entities(&self.world, &self.session_entities)
                    .filter_map(|entity| self.world.get::<ecs::Session>(entity))
                {
                    if let Err(err) = self.cgroup_manager.terminate_session(session_id.0) {
                        log::error!(session_id = session_id.0.as_raw(), error = err.to_string().as_str(); "failed to terminate session before reboot");
                        object.send_error(
                            session_core_v1::ErrorCode::Io,
                            format!(
                                "failed to terminate session {}: {err}",
                                session_id.0.as_raw()
                            ),
                        );
                        return;
                    }
                }

                rustix::fs::sync();

                if let Err(err) = system::reboot(system::RebootCommand::Restart) {
                    log::error!(error = err.to_string().as_str(); "reboot syscall failed");
                    object.send_error(
                        session_core_v1::ErrorCode::Io,
                        format!("failed to reboot: {err}"),
                    );
                    return;
                }

                object.send_done();
            }
            session_core_manager::Event::Poweroff => {
                let Some(client) = object.client() else {
                    return;
                };

                // let user poweroff only if there are no other user sessions active
                let foreign_session_count = session::entities(&self.world, &self.session_entities)
                    .filter_map(|entity| self.world.get::<ecs::Uid>(entity))
                    .filter(|session| session.0 != client.creds().uid.as_raw())
                    .count();

                if !client.creds().uid.is_root() && foreign_session_count > 0 {
                    log::warn!(uid = client.creds().uid.as_raw(), pid = client.creds().pid.as_raw_pid(), foreign_session_count; "poweroff denied: multiple sessions active");
                    object.send_error(
                        session_core_v1::ErrorCode::ActiveSessions,
                        "cannot power off while multiple sessions are active (root privileges required)",
                    );
                    return;
                }

                log::info!(uid = client.creds().uid.as_raw(), pid = client.creds().pid.as_raw_pid(), foreign_session_count; "poweroff accepted");

                for session_id in session::entities(&self.world, &self.session_entities)
                    .filter_map(|entity| self.world.get::<ecs::Session>(entity))
                {
                    if let Err(err) = self.cgroup_manager.terminate_session(session_id.0) {
                        log::error!(session_id = session_id.0.as_raw(), error = err.to_string().as_str(); "failed to terminate session before poweroff");
                        object.send_error(
                            session_core_v1::ErrorCode::Io,
                            format!(
                                "failed to terminate session {}: {err}",
                                session_id.0.as_raw()
                            ),
                        );
                        return;
                    }
                }

                rustix::fs::sync();

                if let Err(err) = system::reboot(system::RebootCommand::PowerOff) {
                    log::error!(error = err.to_string().as_str(); "poweroff syscall failed");
                    object.send_error(
                        session_core_v1::ErrorCode::Io,
                        format!("failed to power off: {err}"),
                    );
                    return;
                }

                object.send_done();
            }
            session_core_manager::Event::Suspend => {
                let Some(client) = object.client() else {
                    return;
                };

                // let user suspend only if there are no other user sessions active
                let foreign_session_count = session::entities(&self.world, &self.session_entities)
                    .filter_map(|entity| self.world.get::<ecs::Uid>(entity))
                    .filter(|session| session.0 != client.creds().uid.as_raw())
                    .count();

                if !client.creds().uid.is_root() && foreign_session_count > 0 {
                    log::warn!(uid = client.creds().uid.as_raw(), pid = client.creds().pid.as_raw_pid(), foreign_session_count; "suspend denied: multiple sessions active");
                    object.send_error(
                        session_core_v1::ErrorCode::ActiveSessions,
                        "cannot suspend while multiple sessions are active (root privileges required)",
                    );
                    return;
                }

                log::info!(uid = client.creds().uid.as_raw(), pid = client.creds().pid.as_raw_pid(), foreign_session_count; "suspend accepted");

                rustix::fs::sync();

                if let Err(err) = fs::write("/sys/power/state", "mem") {
                    log::error!(error = err.to_string().as_str(); "suspend failed");
                    object.send_error(
                        session_core_v1::ErrorCode::Io,
                        format!("failed to suspend: {err}"),
                    );
                    return;
                }

                object.send_done();
            }
            session_core_manager::Event::Hibernate => {
                let Some(client) = object.client() else {
                    return;
                };

                // let user hibernate only if there are no other user sessions active
                let foreign_session_count = session::entities(&self.world, &self.session_entities)
                    .filter_map(|entity| self.world.get::<ecs::Uid>(entity))
                    .filter(|session| session.0 != client.creds().uid.as_raw())
                    .count();

                if !client.creds().uid.is_root() && foreign_session_count > 0 {
                    log::warn!(uid = client.creds().uid.as_raw(), pid = client.creds().pid.as_raw_pid(), foreign_session_count; "hibernate denied: multiple sessions active");
                    object.send_error(
                        session_core_v1::ErrorCode::ActiveSessions,
                        "cannot hibernate while multiple sessions are active (root privileges required)",
                    );
                    return;
                }

                log::info!(uid = client.creds().uid.as_raw(), pid = client.creds().pid.as_raw_pid(), foreign_session_count; "hibernate accepted");

                rustix::fs::sync();

                if let Err(err) = fs::write("/sys/power/state", "disk") {
                    log::error!(error = err.to_string().as_str(); "hibernate failed");
                    object.send_error(
                        session_core_v1::ErrorCode::Io,
                        format!("failed to hibernate: {err}"),
                    );
                    return;
                }

                object.send_done();
            }
            session_core_manager::Event::Destroy => {
                self.info_managers.retain(|m| m != object);
            }
        }
    }
}

struct DaemonConfig {
    socket_path: path::PathBuf,
    vtbound: bool,
    seat_access: SeatAccess,
    loop_signal_tx: Option<std::sync::mpsc::Sender<calloop::LoopSignal>>,
}

fn run(config: DaemonConfig) -> anyhow::Result<()> {
    let mut event_loop = calloop::EventLoop::try_new()?;
    let loop_signal = event_loop.get_signal();

    let cgroup_manager = cgroup::CgroupManager::new(event_loop.handle());

    fs::create_dir_all("/var/lib/sessiond")?;

    let mut sessiond = Sessiond {
        next_session_id: session::SessionId::from_raw(1),
        info_managers: Vec::new(),
        cgroup_manager,
        session_entities: Vec::new(),
        user_entities: Vec::new(),
        seat_entities: Vec::new(),
        client_entities: Vec::new(),
        seat_access: config.seat_access,
        #[cfg(feature = "consolekit")]
        consolekit: consolekit::Server::start(),
        shutting_down: false,
        loop_signal,
        world: world::World::new(),
    };

    let seat_entity = ecs::insert_seat(
        &mut sessiond.world,
        "seat0".to_string(),
        if config.vtbound {
            ecs::SeatKind::VtBound
        } else {
            ecs::SeatKind::NonVtBound
        },
        0,
    );
    sessiond.seat_entities.push(seat_entity);

    match user::lingering_uids() {
        Ok(uids) => {
            for uid in uids {
                user::get_or_create(
                    &mut sessiond.world,
                    &mut sessiond.user_entities,
                    &sessiond.session_entities,
                    uid,
                );
                user::create_runtime_dir(uid);
                if let Err(e) = sessiond.cgroup_manager.ensure_user_cgroup(uid) {
                    log::error!(uid, error = e.to_string().as_str(); "failed to create user cgroup");
                }
            }
        }
        Err(e) => log::error!(error = e.to_string().as_str(); "failed to read lingering users"),
    }

    let path = config.socket_path.as_path();
    let mut socket = server::Server::bind(&path)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o777))?;

    if let Some(tx) = config.loop_signal_tx {
        let _ = tx.send(event_loop.get_signal());
    }

    socket
        .add_implementation::<session_management_v1::SessionManagementV1Impl, _>(&mut sessiond, 1);
    socket.add_implementation::<session_core_v1::SessionCoreV1Impl, _>(&mut sessiond, 1);
    socket.add_implementation::<seat_v1::SeatV1Impl, _>(&mut sessiond, 1);

    let fd_wrapper = unsafe { generic::FdWrapper::new(socket.extract_loop_fd().as_raw_fd()) };
    let source = generic::Generic::new(
        fd_wrapper,
        calloop::Interest {
            readable: true,
            writable: false,
        },
        calloop::Mode::Level,
    );
    event_loop
        .handle()
        .insert_source(source, move |_, _, state| {
            if state.shutting_down {
                return Ok(calloop::PostAction::Continue);
            }

            _ = socket.dispatch_events(state, false);
            Ok(calloop::PostAction::Continue)
        })?;

    let signals = signals::Signals::new(&[
        signals::Signal::SIGUSR1,
        signals::Signal::SIGUSR2,
        signals::Signal::SIGTERM,
        signals::Signal::SIGINT,
    ])?;
    event_loop
        .handle()
        .insert_source(signals, move |event, (), state| {
            let session_updates = match event.signal() {
                signals::Signal::SIGUSR1 | signals::Signal::SIGUSR2 => {
                    seat::handle_vt_signal(&mut state.world, &state.seat_entities, event.signal())
                }
                signals::Signal::SIGTERM | signals::Signal::SIGINT => {
                    if state.shutting_down {
                        return;
                    }

                    state.shutting_down = true;

                    for session_id in session::entities(&state.world, &state.session_entities)
                        .filter_map(|entity| state.world.get::<ecs::Session>(entity))
                    {
                        if let Err(err) = state.cgroup_manager.terminate_session(session_id.0) {
                            log::error!(session_id = session_id.0.as_raw(), error = err.to_string().as_str(); "failed to terminate session during shutdown");
                        }
                    }

                    if state.cgroup_manager.is_empty() {
                        state.loop_signal.stop();
                    }

                    return;
                }
                _ => unreachable!(),
            };

            for (vt, active_state) in session_updates {
                let mut state_updates = Vec::new();

                if active_state == session_core_v1::ActiveState::Active {
                    state_updates.extend(
                        session::entities(&state.world, &state.session_entities).filter_map(
                            |entity| {
                                let session_vt = state.world.get::<ecs::Vt>(entity)?.0;
                                let session_state = state.world.get::<ecs::ActiveState>(entity)?.0;
                                (session_vt != vt
                                    && session_state == session_core_v1::ActiveState::Active)
                                    .then_some((entity, session_core_v1::ActiveState::Online))
                            },
                        ),
                    );
                }

                state_updates.extend(
                    session::entities(&state.world, &state.session_entities).filter_map(|entity| {
                        let session_vt = state.world.get::<ecs::Vt>(entity)?.0;
                        (session_vt == vt).then_some((entity, active_state))
                    }),
                );

                for (entity, active_state) in state_updates {
                    let Some(session_id) = session::id(&state.world, entity) else {
                        continue;
                    };
                    session::set_state(&mut state.world, session_id, entity, active_state);
                    #[cfg(feature = "consolekit")]
                    state.sync_consolekit_session(session_id, entity);
                }
            }

            user::sync_all(&mut state.world, &state.user_entities, &state.session_entities);
        })?;

    event_loop.run(None, &mut sessiond, |_| {})?;

    let _ = fs::remove_file(&config.socket_path);

    Ok(())
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let log_level = match env::var("LOG_LEVEL") {
        Ok(log) if log == "trace" => log::LevelFilter::Trace,
        Ok(log) if log == "debug" => log::LevelFilter::Debug,
        Ok(log) if log == "warn" => log::LevelFilter::Warn,
        Ok(log) if log == "error" => log::LevelFilter::Error,
        Ok(log) if log == "off" => log::LevelFilter::Off,
        _ => log::LevelFilter::Info,
    };

    env_logger::Builder::new()
        .filter(Some("daemon"), log_level)
        .init();

    log::info!("sessiond daemon starting");

    let vtbound = env::var("SESSIOND_VTBOUND")
        .ok()
        .is_none_or(|vtbound| vtbound == "1");

    run(DaemonConfig {
        socket_path: path::PathBuf::from("/run/sessiond.sock"),
        vtbound,
        seat_access: SeatAccess::new(
            resolve_user(&args.seat_user)?,
            resolve_group(&args.seat_group)?,
        ),
        loop_signal_tx: None,
    })
}

fn resolve_user(user: &str) -> anyhow::Result<process::Uid> {
    if let Ok(uid) = user.parse::<process::RawUid>() {
        return Ok(process::Uid::from_raw(uid));
    }

    users::get_user_by_name(user)
        .map(|user: users::User| process::Uid::from_raw(user.uid()))
        .ok_or_else(|| anyhow::anyhow!("unknown seat user {user:?}"))
}

fn resolve_group(group: &str) -> anyhow::Result<process::Gid> {
    if let Ok(gid) = group.parse::<process::RawGid>() {
        return Ok(process::Gid::from_raw(gid));
    }

    users::get_group_by_name(group)
        .map(|group: users::Group| process::Gid::from_raw(group.gid()))
        .ok_or_else(|| anyhow::anyhow!("unknown seat group {group:?}"))
}
