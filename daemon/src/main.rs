mod session_management_v1 {
    hyprwire::include_protocol!("session_management_v1");
    pub use server::*;
    pub use spec::*;
}
mod session_management_v2 {
    hyprwire::include_protocol!("session_management_v2");
    pub use server::*;
    pub use spec::*;
}
mod session_core_v1 {
    hyprwire::include_protocol!("session_core_v1");
    pub use server::*;
    pub use spec::*;
}

mod cgroup;
mod config;
#[cfg(feature = "consolekit")]
mod consolekit;
mod ecs;
mod reconcile;
mod session;
mod terminal;
mod user;
mod vt;

#[cfg(test)]
mod tests;

use bevy_ecs::prelude::*;
#[cfg(feature = "consolekit")]
use calloop::channel;
use calloop::{generic, signals};
use clap::{Parser, ValueEnum};
use hyprwire::server;
use rustix::{net, process};
use session_core_v1::session_core_user;
use std::os::fd::AsRawFd;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::PermissionsExt;
use std::{env, fs, path};

#[derive(Parser)]
struct Args {
    #[arg(long, default_value = "/run/sessiond.sock")]
    socket_path: path::PathBuf,
    #[arg(short, long, help = "Path to configuration file")]
    config: Option<path::PathBuf>,
    #[arg(long, value_enum, default_value_t = LogTarget::Stderr)]
    log_target: LogTarget,
}

#[derive(Clone, Copy, ValueEnum)]
enum LogTarget {
    Stderr,
    Syslog,
}

#[derive(Clone, Copy)]
struct Access {
    user: process::Uid,
    group: process::Gid,
}

impl Access {
    const fn new(user: process::Uid, group: process::Gid) -> Self {
        Self { user, group }
    }

    fn allows(self, creds: &hyprwire::Creds) -> bool {
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
    cgroup_manager: cgroup::CgroupManager,
    #[cfg(feature = "consolekit")]
    consolekit: consolekit::Server,
    shutting_down: bool,
    loop_signal: calloop::LoopSignal,
    world: World,
    notify_schedule: Schedule,
}

impl Sessiond {
    fn remove_session(&mut self, session_id: session::SessionId) -> Option<Entity> {
        let entity = session::entity(&self.world, session_id)?;
        self.world
            .resource_mut::<ecs::ProtocolObjects>()
            .remove_session_objects(entity);
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

fn notify(message: &[u8]) -> rustix::io::Result<()> {
    let Some(socket_path) = env::var_os("NOTIFY_SOCKET") else {
        return Ok(());
    };

    let address = socket_path.as_bytes().strip_prefix(b"@").map_or_else(
        || net::SocketAddrUnix::new(&socket_path),
        net::SocketAddrUnix::new_abstract_name,
    )?;
    let socket = net::socket(net::AddressFamily::UNIX, net::SocketType::DGRAM, None)?;
    net::connect(&socket, &address)?;
    net::send(&socket, message, net::SendFlags::empty())?;
    Ok(())
}

fn run(socket_path: &path::Path, config: config::Config) -> anyhow::Result<()> {
    let mut event_loop = calloop::EventLoop::try_new()?;
    let loop_signal = event_loop.get_signal();
    #[cfg(feature = "consolekit")]
    let (consolekit_power_sender, consolekit_power_channel) = channel::channel();
    let signals = signals::Signals::new(&[
        signals::Signal::SIGUSR1,
        signals::Signal::SIGUSR2,
        signals::Signal::SIGTERM,
        signals::Signal::SIGINT,
    ])?;

    let access_rules = ecs::AccessRules {
        management: Access::new(
            resolve_user(&config.management.user)?,
            resolve_group(&config.management.group)?,
        ),
    };

    let lingering_users = config
        .users
        .iter()
        .filter(|(_, config)| config.linger)
        .filter_map(|(name, config)| {
            let user = uzers::get_user_by_name(name)?;
            let uid = user.uid();
            Some((uid, config.runtime_dir(uid)))
        })
        .collect::<Vec<_>>();

    let mut sessiond = Sessiond {
        cgroup_manager: cgroup::CgroupManager::new(event_loop.handle())?,
        #[cfg(feature = "consolekit")]
        consolekit: consolekit::Server::start(consolekit_power_sender),
        shutting_down: false,
        loop_signal,
        world: World::new(),
        notify_schedule: reconcile::notify_schedule(),
    };

    sessiond
        .world
        .insert_resource(ecs::PendingSessionEnvs::default());
    sessiond
        .world
        .insert_resource(ecs::NextSessionId(session::SessionId::from_raw(1)));
    sessiond
        .world
        .insert_resource(ecs::SessionInfoManagers::default());
    ecs::init_resources(&mut sessiond.world);

    sessiond.world.insert_resource(access_rules);
    sessiond.world.insert_resource(ecs::DaemonConfig(config));

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
    sessiond.world.clear_trackers();

    let mut socket = server::Server::bind(&socket_path)?;
    fs::set_permissions(socket_path, fs::Permissions::from_mode(0o777))?;

    socket.add_implementation::<session_management_v1::SessionManagementV1, _>(&mut sessiond, 1);
    socket.add_implementation::<session_management_v2::SessionManagementV2, _>(&mut sessiond, 1);
    socket.add_implementation::<session_core_v1::SessionCoreV1, _>(&mut sessiond, 1);

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
        })
        .map_err(|err| anyhow::anyhow!("failed to insert VT watcher: {err:?}"))?;

    #[cfg(feature = "consolekit")]
    event_loop
        .handle()
        .insert_source(consolekit_power_channel, move |event, (), state| {
            let channel::Event::Msg(request) = event else {
                return;
            };

            match request.kind {
                consolekit::PowerRequestKind::Execute(action) => {
                    let result = state.handle_power_action(action, &request.creds);
                    let _ = request.reply.send(consolekit::PowerResponse::Done(result));
                }
                consolekit::PowerRequestKind::Can(action) => {
                    let _ = action;
                    let can = state.can_control_power(&request.creds);
                    let _ = request.reply.send(consolekit::PowerResponse::Can(can));
                }
            }
        })
        .map_err(|err| anyhow::anyhow!("failed to insert ConsoleKit power source: {err}"))?;

    event_loop
        .handle()
        .insert_source(signals, move |event, (), state| {
            match event.signal() {
                signals::Signal::SIGUSR1 | signals::Signal::SIGUSR2 => {
                    let terminal = match terminal::Terminal::open(0) {
                        Ok(terminal) => terminal,
                        Err(error) => {
                            log::error!(
                                error = error.to_string().as_str();
                                "failed to open terminal to handle VT signal"
                            );
                            return;
                        }
                    };

                    match event.signal() {
                        signals::Signal::SIGUSR1 => {
                            if let Err(error) = terminal.ack_release() {
                                log::error!(
                                    error = error.to_string().as_str();
                                    "failed to acknowledge VT release"
                                );
                            }
                            return;
                        }
                        signals::Signal::SIGUSR2 => {
                            if let Err(error) = terminal.ack_acquire() {
                                log::error!(
                                    error = error.to_string().as_str();
                                    "failed to acknowledge VT acquire"
                                );
                            }
                        }
                        _ => return,
                    }
                    let Ok(current) = vt::current_vt() else {
                        return;
                    };

                    {
                        let mut sessions = state.world.query_filtered::<(
                            &ecs::Vt,
                            &ecs::IsLocal,
                            &mut ecs::ActiveState,
                        ), With<ecs::Session>>(
                        );
                        for (vt, _, mut active_state) in sessions.iter_mut(&mut state.world) {
                            let desired = if vt.0 == current {
                                session_core_v1::ActiveState::Active
                            } else {
                                session_core_v1::ActiveState::Online
                            };
                            if active_state.0 != desired {
                                active_state.0 = desired;
                            }
                        }
                    }

                    // TODO: move this into ECS reconciliation
                    #[cfg(feature = "consolekit")]
                    for entity in session::entities(&state.world) {
                        if let Some(session_id) = session::id(&state.world, entity) {
                            state.sync_consolekit_session(session_id, entity);
                        }
                    }
                }
                signals::Signal::SIGTERM | signals::Signal::SIGINT => {
                    if state.shutting_down {
                        return;
                    }

                    if let Err(e) = notify(b"STOPPING=1\n") {
                        log::warn!("Failed to send shutdown notification: {e}");
                    }

                    state.shutting_down = true;

                    for session_id in session::entities(&state.world)
                        .into_iter()
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
            }

            user::sync_all(&mut state.world);
        })?;

    terminal::Terminal::open(0)?.set_process_switching(true)?;

    if let Err(e) = notify(b"READY=1\n") {
        log::warn!("Failed to send readiness notification: {e}");
    }

    event_loop.run(None, &mut sessiond, |state| {
        state.notify_schedule.run(&mut state.world);
    })?;

    terminal::Terminal::open(0)?.set_process_switching(false)?;

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

    match args.log_target {
        LogTarget::Stderr => {
            env_logger::Builder::new()
                .filter(Some("daemon"), log_level)
                .init();
        }
        LogTarget::Syslog => {
            syslog::init(syslog::Facility::LOG_DAEMON, log_level, Some("sessiond"))?;
        }
    }

    log::info!("sessiond daemon starting");

    run(&args.socket_path, config::Config::load(args.config)?)
}

fn resolve_user(user: &str) -> anyhow::Result<process::Uid> {
    if let Ok(uid) = user.parse::<process::RawUid>() {
        return Ok(process::Uid::from_raw(uid));
    }

    uzers::get_user_by_name(user)
        .map(|user: uzers::User| process::Uid::from_raw(user.uid()))
        .ok_or_else(|| anyhow::anyhow!("unknown user {user:?}"))
}

fn resolve_group(group: &str) -> anyhow::Result<process::Gid> {
    if let Ok(gid) = group.parse::<process::RawGid>() {
        return Ok(process::Gid::from_raw(gid));
    }

    uzers::get_group_by_name(group)
        .map(|group: uzers::Group| process::Gid::from_raw(group.gid()))
        .ok_or_else(|| anyhow::anyhow!("unknown group {group:?}"))
}
