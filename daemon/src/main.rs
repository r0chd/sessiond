mod session_management_v1 {
    hyprwire::include_protocol!("session_management_v1");
    pub use server::*;
}
mod session_management_v2 {
    hyprwire::include_protocol!("session_management_v2");
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
mod tests;

use bevy_ecs::prelude::*;
use calloop::{generic, signals};
use clap::Parser;
use hyprwire::server;
use rustix::{net, process};
use session_core_v1::{session_core_manager, session_core_session, session_core_user};
use session_management_v1::session_manager;
use std::collections::{BTreeMap, HashMap};
use std::os::fd::AsRawFd;
use std::os::unix::fs::PermissionsExt;
use std::{env, fs, path};

#[derive(Parser)]
struct Args {
    #[arg(long, default_value = "/run/sessiond.sock")]
    socket_path: path::PathBuf,
    #[arg(short, long, help = "Path to configuration file")]
    config: Option<path::PathBuf>,
}

struct Access {
    user: process::Uid,
    group: process::Gid,
}

impl Access {
    const fn new(user: process::Uid, group: process::Gid) -> Self {
        Self { user, group }
    }

    fn allows(&self, creds: &net::UCred) -> bool {
        if self.user == creds.uid || self.group == creds.gid || creds.uid.is_root() {
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
    envs: HashMap<session_manager::SessionManager, BTreeMap<String, String>>,
    cgroup_manager: cgroup::CgroupManager,
    seat_access: Access,
    management_access: Access,
    settings: config::Config,
    #[cfg(feature = "consolekit")]
    consolekit: consolekit::Server,
    shutting_down: bool,
    loop_signal: calloop::LoopSignal,
    world: World,
}

impl Sessiond {
    fn remove_session(&mut self, session_id: session::SessionId) -> Option<Entity> {
        let entity = session::entity(&self.world, session_id)?;
        self.world.despawn(entity);
        #[cfg(feature = "consolekit")]
        self.consolekit.remove_session(session_id);
        Some(entity)
    }

    #[cfg(feature = "consolekit")]
    fn sync_consolekit_session(&self, session_id: session::SessionId, entity: Entity) {
        if let Some(info) = consolekit::SessionInfo::from_world(&self.world, entity, session_id) {
            self.consolekit.upsert_session(info);
        }
    }
}

struct DaemonConfig {
    settings: config::Config,
    socket_path: path::PathBuf,
}

fn run(config: DaemonConfig) -> anyhow::Result<()> {
    let mut event_loop = calloop::EventLoop::try_new()?;
    let loop_signal = event_loop.get_signal();
    let signals = signals::Signals::new(&[
        signals::Signal::SIGUSR1,
        signals::Signal::SIGUSR2,
        signals::Signal::SIGTERM,
        signals::Signal::SIGINT,
    ])?;

    anyhow::ensure!(
        !config.settings.seats.is_empty(),
        "exactly one seat must be configured"
    );
    let seat_config = config
        .settings
        .seats
        .get(&config.settings.default_seat)
        .ok_or_else(|| anyhow::anyhow!("configured `default-seat` not found"))?;

    let vtbound = seat_config.kind == config::SeatKind::VtBound;

    let mut sessiond = Sessiond {
        next_session_id: session::SessionId::from_raw(1),
        info_managers: Vec::new(),
        envs: HashMap::new(),
        cgroup_manager: cgroup::CgroupManager::new(event_loop.handle())?,
        seat_access: Access::new(
            resolve_user(&seat_config.user)?,
            resolve_group(&seat_config.group)?,
        ),
        management_access: Access::new(
            resolve_user(&config.settings.management.user)?,
            resolve_group(&config.settings.management.group)?,
        ),
        settings: config.settings,
        #[cfg(feature = "consolekit")]
        consolekit: consolekit::Server::start(),
        shutting_down: false,
        loop_signal,
        world: World::new(),
    };

    seat::spawn(
        &mut sessiond.world,
        sessiond.settings.default_seat.clone(),
        if vtbound {
            ecs::SeatKind::VtBound
        } else {
            ecs::SeatKind::NonVtBound
        },
        0,
    );

    let lingering_users = sessiond
        .settings
        .users
        .iter()
        .filter(|(_, config)| config.linger)
        .filter_map(|(name, config)| {
            let user = uzers::get_user_by_name(name)?;
            let uid = user.uid();
            Some((uid, config.runtime_dir(uid)))
        })
        .collect::<Vec<_>>();

    for (uid, runtime_dir) in lingering_users {
        user::create(&mut sessiond.world, uid, true);
        user::create_runtime_dir(uid, &runtime_dir);
        if let Err(e) = sessiond.cgroup_manager.ensure_user_cgroup(uid) {
            log::error!(
                uid,
                error = e.to_string().as_str();
                "failed to create user cgroup"
            );
        }
    }

    let path = config.socket_path.as_path();
    let mut socket = server::Server::bind(&path)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o777))?;

    socket
        .add_implementation::<session_management_v1::SessionManagementV1Impl, _>(&mut sessiond, 1);
    socket
        .add_implementation::<session_management_v2::SessionManagementV2Impl, _>(&mut sessiond, 1);
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
                    seat::handle_vt_signal(&mut state.world, event.signal())
                }
                signals::Signal::SIGTERM | signals::Signal::SIGINT => {
                    if state.shutting_down {
                        return;
                    }

                    state.shutting_down = true;

                    for session_id in session::entities(&state.world)
                        .filter_map(|entity| state.world.get::<ecs::SessionId>(entity))
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
                    state_updates.extend(session::entities(&state.world).filter_map(|entity| {
                        let session_vt = state.world.get::<ecs::Vt>(entity)?.0.get();
                        let session_state = state.world.get::<ecs::ActiveState>(entity)?.0;

                        if session_vt != vt.get()
                            && session_state == session_core_v1::ActiveState::Active
                        {
                            return Some((entity, session_core_v1::ActiveState::Online));
                        }

                        None
                    }));
                }

                state_updates.extend(session::entities(&state.world).filter_map(|entity| {
                    let session_vt = state.world.get::<ecs::Vt>(entity)?.0.get();
                    (session_vt == vt.get()).then_some((entity, active_state))
                }));

                for (entity, active_state) in state_updates {
                    let Some(session_id) = session::id(&state.world, entity) else {
                        continue;
                    };
                    session::set_state(&mut state.world, session_id, entity, active_state);
                    #[cfg(feature = "consolekit")]
                    state.sync_consolekit_session(session_id, entity);
                }
            }

            user::sync_all(&mut state.world);
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

    let config = DaemonConfig {
        settings: config::Config::load(args.config)?,
        socket_path: args.socket_path,
    };

    run(config)
}

fn resolve_user(user: &str) -> anyhow::Result<process::Uid> {
    if let Ok(uid) = user.parse::<process::RawUid>() {
        return Ok(process::Uid::from_raw(uid));
    }

    uzers::get_user_by_name(user)
        .map(|user: uzers::User| process::Uid::from_raw(user.uid()))
        .ok_or_else(|| anyhow::anyhow!("unknown seat user {user:?}"))
}

fn resolve_group(group: &str) -> anyhow::Result<process::Gid> {
    if let Ok(gid) = group.parse::<process::RawGid>() {
        return Ok(process::Gid::from_raw(gid));
    }

    uzers::get_group_by_name(group)
        .map(|group: uzers::Group| process::Gid::from_raw(group.gid()))
        .ok_or_else(|| anyhow::anyhow!("unknown seat group {group:?}"))
}
