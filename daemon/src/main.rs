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
mod client;
mod device;
mod ecs;
mod seat;
mod session;
mod terminal;

use calloop::{generic, signals};
use clap::Parser;
use hyprwire::server;
use rustix::{net, process};
use session_core_v1::{session_core_manager, session_core_session};
use session_management_v1::session_manager;
use std::os::fd::AsRawFd;
use std::os::unix::fs::PermissionsExt;
use std::{cell, env, fs, path, rc, time};

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
    seat: Vec<seat::Seat>,
    clients: Vec<rc::Rc<cell::RefCell<client::Client>>>,
    seat_access: SeatAccess,
    world: ecs::World,
}

impl Sessiond {
    fn add_session_object(
        &mut self,
        session_id: session::SessionId,
        entity: ecs::Entity,
        object: session_core_session::SessionCoreSession,
    ) {
        log::debug!(session_id = session_id.as_raw(), uid = object.client().map_or(0, |c| c.creds().uid.as_raw()); "session info requested");

        object.send_session_id(session_id.as_raw());
        if let Some(uid) = self.world.get::<ecs::Uid>(entity) {
            object.send_uid(uid.0);
        }
        if let Some(username) = self.world.get::<ecs::Username>(entity) {
            object.send_username(&username.0);
        }
        if let Some(vt) = self.world.get::<ecs::Vt>(entity) {
            object.send_vt(vt.0);
        }
        if let Some(active_state) = self.world.get::<ecs::ActiveState>(entity) {
            object.send_active(active_state.0);
        }
        if let Some(seat) = self
            .world
            .get::<ecs::Seat>(entity)
            .and_then(|seat| seat.0.as_ref())
        {
            object.send_seat(seat);
        }
        if let Some(leader_id) = self.world.get::<ecs::LeaderId>(entity) {
            object.send_leader(leader_id.0.cast_unsigned());
        }
        if let Some(timestamp) = self.world.get::<ecs::Timestamp>(entity) {
            let timestamp_secs = timestamp
                .0
                .duration_since(time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let timestamp = u32::try_from(timestamp_secs).unwrap_or(u32::MAX);
            object.send_timestamp(timestamp);
        }
        object.send_done();

        if let Some(objects) = self.world.get_mut::<ecs::SessionObjects>(entity) {
            objects.0.push(object);
        }
    }

    pub(crate) fn set_session_state(
        &mut self,
        session_id: session::SessionId,
        entity: ecs::Entity,
        state: session_core_v1::ActiveState,
    ) {
        let vt = self.world.get::<ecs::Vt>(entity).map_or(0, |vt| vt.0);
        log::info!(session_id = session_id.as_raw(), vt; "session state changed");
        if let Some(active_state) = self.world.get_mut::<ecs::ActiveState>(entity) {
            active_state.0 = state;
        }
        if let Some(objects) = self.world.get::<ecs::SessionObjects>(entity) {
            for object in &objects.0 {
                object.send_active(state);
            }
        }
    }

    fn sync_vt_session_states(&mut self) {
        let active_vts = self
            .seat
            .iter_mut()
            .filter_map(|seat| {
                seat.active_vt()
                    .inspect_err(|err| log::error!("Could not query active VT: {err}"))
                    .ok()
                    .flatten()
            })
            .collect::<Vec<_>>();

        if active_vts.is_empty() {
            return;
        }

        for session_id in self.world.session_ids().collect::<Vec<_>>() {
            let Some(entity) = self.world.session_entity(session_id) else {
                continue;
            };
            let Some(active_state) = self
                .world
                .get::<ecs::ActiveState>(entity)
                .map(|state| state.0)
            else {
                continue;
            };
            if matches!(
                active_state,
                session_core_v1::ActiveState::Opening | session_core_v1::ActiveState::Closing
            ) {
                continue;
            }

            let Some(vt) = self.world.get::<ecs::Vt>(entity).map(|vt| vt.0) else {
                continue;
            };
            let state = if active_vts.contains(&vt) {
                session_core_v1::ActiveState::Active
            } else {
                session_core_v1::ActiveState::Online
            };

            if active_state != state {
                session::set_state(&mut self.world, session_id, entity, state);
            }
        }
    }
}

impl session_management_v1::SessionManagementV1Handler for Sessiond {}

impl session_core_v1::SessionCoreV1Handler for Sessiond {
    fn bind(&mut self, object: session_core_manager::SessionCoreManager) {
        self.sync_vt_session_states();

        let session_ids = self
            .world
            .session_ids()
            .map(|session| session.as_raw())
            .collect::<Vec<_>>();
        object.send_sessions(&session_ids);
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

        if !client.creds().uid.is_root() {
            log::warn!(uid = client.creds().uid.as_raw(); "rejected request from non-root client");
            return;
        }

        match event {
            session_manager::Event::CreateSession { uid, username, vt } => {
                let session_id = self.next_session_id;
                self.next_session_id = self.next_session_id.next();
                log::info!(session_id = session_id.as_raw(), uid, username = username.as_str(), vt; "session created");

                let leader_id = client.creds().pid.as_raw_pid();

                if let Err(e) = self
                    .cgroup_manager
                    .create_session_cgroup(session_id, leader_id)
                {
                    log::error!(session_id = session_id.as_raw(), error = e.to_string().as_str(); "failed to create session cgroup");
                }

                let mut session_seat = self
                    .seat
                    .first()
                    .map(|seat| seat.name().to_string())
                    .filter(|seat| !seat.is_empty());
                let mut active_state = session_core_v1::ActiveState::Online;

                let runtime_dir = format!("/run/user/{uid}");
                if let Err(e) = fs::create_dir_all(&runtime_dir) {
                    log::error!(uid, error = e.to_string().as_str(); "failed to create runtime dir");
                }
                if let Err(e) =
                    rustix::fs::chown(&runtime_dir, Some(rustix::fs::Uid::from_raw(uid)), None)
                {
                    log::error!(uid, error = e.to_string().as_str(); "failed to chown runtime dir");
                }
                if let Err(e) =
                    rustix::fs::chmod(&runtime_dir, rustix::fs::Mode::from_bits_truncate(0o700))
                {
                    log::error!(uid, error = e.to_string().as_str(); "failed to chmod runtime dir");
                }

                if let Some(seat) = self
                    .seat
                    .iter_mut()
                    .filter(|seat| seat.is_vt_bound())
                    .find_map(|seat| match seat.is_vt_active(vt) {
                        Ok(true) => Some(seat.name().to_string()),
                        Ok(false) => None,
                        Err(err) => {
                            log::error!(
                                session_id = session_id.as_raw(),
                                vt,
                                error = err.to_string().as_str();
                                "failed to check active VT"
                            );
                            None
                        }
                    })
                {
                    session_seat = Some(seat);
                    active_state = session_core_v1::ActiveState::Active;
                }

                self.world.insert_session(
                    session_id,
                    uid,
                    username,
                    vt,
                    session_seat.clone(),
                    time::SystemTime::now(),
                    leader_id,
                    Vec::new(),
                    active_state,
                );

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
                let Some(entity) = self.world.session_entity(session_id) else {
                    log::warn!(session_id = session_id.as_raw(); "destroy requested for unknown session");
                    return;
                };

                let uid = self.world.get::<ecs::Uid>(entity).map_or(0, |uid| uid.0);
                let username = self
                    .world
                    .get::<ecs::Username>(entity)
                    .map_or("", |username| username.0.as_str());
                log::info!(session_id = session_id.as_raw(), uid, username; "session destroyed");

                if let Some(active_state) = self.world.get_mut::<ecs::ActiveState>(entity) {
                    active_state.0 = session_core_v1::ActiveState::Closing;
                }
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
                let _ = self.world.remove_session(session_id);

                for manager in &self.info_managers {
                    manager.send_session_removed(session_id.as_raw());
                }

                if let Err(e) = self.cgroup_manager.poll_destroy(session_id) {
                    log::error!(session_id = session_id.as_raw(), error = e.to_string().as_str(); "failed to poll destroy session cgroup");
                }

                if !self.world.session_ids().any(|session_id| {
                    self.world
                        .session_entity(session_id)
                        .and_then(|entity| self.world.get::<ecs::Uid>(entity))
                        .is_some_and(|session_uid| session_uid.0 == uid)
                }) && let Err(e) = fs::remove_dir(format!("/run/user/{uid}"))
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
                self.sync_vt_session_states();

                let session_id = session::SessionId::from_raw(session_id);
                if let Some(entity) = self.world.session_entity(session_id) {
                    self.add_session_object(session_id, entity, session_core_session);
                    return;
                }

                log::warn!(session_id = session_id.as_raw(); "get_session requested for unknown session");
                session_core_session.send_destroyed();
            }
            session_core_manager::Event::GetSessionByPid {
                session_core_session,
                pid,
            } => {
                self.sync_vt_session_states();

                let Ok(Some(session_id)) = self.cgroup_manager.find_session_by_pid(pid) else {
                    log::warn!(pid; "get_session_by_pid: no session found for pid");
                    session_core_session.send_destroyed();
                    return;
                };

                if let Some(entity) = self.world.session_entity(session_id) {
                    self.add_session_object(session_id, entity, session_core_session);
                    return;
                }

                log::warn!(session_id = session_id.as_raw(); "get_session_by_pid requested for unknown session");
                session_core_session.send_destroyed();
            }
            session_core_manager::Event::GetSessionByUid {
                session_core_session,
                uid,
            } => {
                self.sync_vt_session_states();

                if let Some((session_id, entity)) = self.world.session_entity_by_uid(uid) {
                    self.add_session_object(session_id, entity, session_core_session);
                    return;
                }

                log::warn!(uid; "get_session_by_uid requested for unknown session");
                session_core_session.send_destroyed();
            }
            session_core_manager::Event::Destroy => {
                self.info_managers.retain(|m| m != object);
            }
        }
    }
}

#[allow(clippy::too_many_lines)]
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

    let seat_access = SeatAccess::new(
        resolve_user(&args.seat_user)?,
        resolve_group(&args.seat_group)?,
    );

    let mut event_loop = calloop::EventLoop::try_new()?;

    let cgroup_manager = cgroup::CgroupManager::new(event_loop.handle())?;

    let vtbound = env::var("SESSIOND_VTBOUND")
        .ok()
        .is_none_or(|vtbound| vtbound == "1");

    let seat = if vtbound {
        seat::Seat::vt_bound("seat0".to_string())
    } else {
        seat::Seat::non_vt_bound("seat0".to_string())
    };

    let mut sessiond = Sessiond {
        next_session_id: session::SessionId::from_raw(1),
        info_managers: Vec::new(),
        cgroup_manager,
        seat: vec![seat],
        clients: Vec::new(),
        seat_access,
        world: ecs::World::new(),
    };

    let path = path::Path::new("/run/sessiond.sock");
    let mut socket = server::Server::bind(&path)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o777))?;

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
            _ = socket.dispatch_events(state, false);
            Ok(calloop::PostAction::Continue)
        })?;

    let signals = signals::Signals::new(&[signals::Signal::SIGUSR1, signals::Signal::SIGUSR2])?;
    event_loop
        .handle()
        .insert_source(signals, move |event, (), state| {
            let mut session_updates = Vec::new();

            for seat in state.seat.iter_mut().filter(|seat| seat.is_vt_bound()) {
                match event.signal() {
                    signals::Signal::SIGUSR1 => match seat.vt_release() {
                        Ok(vt) => {
                            session_updates.push((vt, session_core_v1::ActiveState::Online));
                        }
                        Err(e) => {
                            log::error!("Could not release VT: {e}");
                        }
                    },
                    signals::Signal::SIGUSR2 => match seat.vt_activate() {
                        Ok(vt) => {
                            session_updates.push((vt, session_core_v1::ActiveState::Active));
                        }
                        Err(e) => {
                            log::error!("Could not activate VT: {e}");
                        }
                    },
                    _ => unreachable!(),
                }
            }

            for (vt, active_state) in session_updates {
                if active_state == session_core_v1::ActiveState::Active {
                    for session_id in state.world.session_ids().collect::<Vec<_>>() {
                        let Some(entity) = state.world.session_entity(session_id) else {
                            continue;
                        };
                        let Some(session_vt) = state.world.get::<ecs::Vt>(entity).map(|vt| vt.0)
                        else {
                            continue;
                        };
                        let Some(session_state) = state
                            .world
                            .get::<ecs::ActiveState>(entity)
                            .map(|state| state.0)
                        else {
                            continue;
                        };
                        if session_vt != vt && session_state == session_core_v1::ActiveState::Active
                        {
                            session::set_state(
                                &mut state.world,
                                session_id,
                                entity,
                                session_core_v1::ActiveState::Online,
                            );
                        }
                    }
                }

                for session_id in state.world.session_ids().collect::<Vec<_>>() {
                    let Some(entity) = state.world.session_entity(session_id) else {
                        continue;
                    };
                    let Some(session_vt) = state.world.get::<ecs::Vt>(entity).map(|vt| vt.0) else {
                        continue;
                    };
                    if session_vt == vt {
                        session::set_state(&mut state.world, session_id, entity, active_state);
                    }
                }
            }
        })?;

    event_loop.run(None, &mut sessiond, |_| {})?;

    Ok(())
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
