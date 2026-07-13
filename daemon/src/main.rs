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
mod config;
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
use rustix::{net, process};
use session_core_v1::{session_core_manager, session_core_session, session_core_user};
use std::os::fd::AsRawFd;
use std::os::unix::fs::PermissionsExt;
use std::{env, fs, path};

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
    #[arg(long, default_value = "/run/sessiond.sock")]
    socket_path: path::PathBuf,
    #[arg(
        long,
        value_name = "USER",
        default_value = "root",
        help = "Allow USER to create and destroy sessions"
    )]
    management_user: String,
    #[arg(
        long,
        help = "Disable cgroup management (useful for running without root)"
    )]
    no_cgroup: bool,
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

struct Sessiond<C: cgroup::CgroupManager + 'static> {
    next_session_id: session::SessionId,
    info_managers: Vec<session_core_manager::SessionCoreManager>,
    cgroup_manager: C,
    session_entities: Vec<entity::Entity>,
    user_entities: Vec<entity::Entity>,
    seat_entities: Vec<entity::Entity>,
    client_entities: Vec<entity::Entity>,
    seat_access: SeatAccess,
    management_uid: process::Uid,
    #[cfg(feature = "consolekit")]
    consolekit: consolekit::Server,
    shutting_down: bool,
    loop_signal: calloop::LoopSignal,
    world: world::World,
}

impl<C: cgroup::CgroupManager + 'static> Sessiond<C> {
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

struct DaemonConfig {
    socket_path: path::PathBuf,
    vtbound: bool,
    seat_access: SeatAccess,
    management_uid: process::Uid,
}

fn run<C: cgroup::CgroupManager + 'static>(
    config: DaemonConfig,
    make_cgroup: impl FnOnce(calloop::LoopHandle<'static, Sessiond<C>>) -> anyhow::Result<C>,
) -> anyhow::Result<()> {
    let mut event_loop = calloop::EventLoop::try_new()?;
    let loop_signal = event_loop.get_signal();
    let signals = signals::Signals::new(&[
        signals::Signal::SIGUSR1,
        signals::Signal::SIGUSR2,
        signals::Signal::SIGTERM,
        signals::Signal::SIGINT,
    ])?;

    let cgroup_manager = make_cgroup(event_loop.handle())?;

    if let Err(e) = fs::create_dir_all("/var/lib/sessiond") {
        log::warn!(
            error = e.to_string().as_str();
            "failed to create linger dir, linger feature unavailable"
        );
    }

    let mut sessiond = Sessiond {
        next_session_id: session::SessionId::from_raw(1),
        info_managers: Vec::new(),
        cgroup_manager,
        session_entities: Vec::new(),
        user_entities: Vec::new(),
        seat_entities: Vec::new(),
        client_entities: Vec::new(),
        seat_access: config.seat_access,
        management_uid: config.management_uid,
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
                    log::error!(
                        uid,
                        error = e.to_string().as_str();
                        "failed to create user cgroup"
                    );
                }
            }
        }
        Err(e) => log::error!(error = e.to_string().as_str(); "failed to read lingering users"),
    }

    let path = config.socket_path.as_path();
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
            if state.shutting_down {
                return Ok(calloop::PostAction::Continue);
            }

            _ = socket.dispatch_events(state, false);
            Ok(calloop::PostAction::Continue)
        })?;

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
                            log::error!(
                                session_id = session_id.0.as_raw(),
                                error = err.to_string().as_str();
                                "failed to terminate session during shutdown"
                            );
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

                                match session_vt {
                                    Some(session_vt)
                                        if session_vt != vt
                                            && session_state
                                                == session_core_v1::ActiveState::Active =>
                                    {
                                        Some((entity, session_core_v1::ActiveState::Online))
                                    }
                                    None => Some((entity, session_core_v1::ActiveState::Active)),
                                    _ => None,
                                }
                            },
                        ),
                    );
                }

                state_updates.extend(
                    session::entities(&state.world, &state.session_entities).filter_map(|entity| {
                        let session_vt = state.world.get::<ecs::Vt>(entity)?.0;
                        session_vt
                            .is_some_and(|session_vt| session_vt == vt)
                            .then_some((entity, active_state))
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

            user::sync_all(
                &mut state.world,
                &state.user_entities,
                &state.session_entities,
            );
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

    let config = DaemonConfig {
        socket_path: args.socket_path,
        vtbound,
        seat_access: SeatAccess::new(
            resolve_user(&args.seat_user)?,
            resolve_group(&args.seat_group)?,
        ),
        management_uid: resolve_user(&args.management_user)?,
    };

    if args.no_cgroup {
        run(config, |_| Ok(cgroup::DummyCgroupManager))
    } else {
        run(config, cgroup::SystemCgroupManager::new)
    }
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
