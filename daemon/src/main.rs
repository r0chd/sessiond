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
mod session_power_v1 {
    hyprwire::include_protocol!("session_power_v1");
    pub use server::*;
    pub use spec::*;
}

mod access;
mod cgroup;
mod config;
#[cfg(feature = "consolekit")]
mod consolekit;
mod power;
mod reconcile;
mod session;
mod state;
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
use std::{env, fs, num, path};

#[derive(Parser)]
struct Args {
    #[arg(long, default_value = "/run/sessiond")]
    runtime_dir: path::PathBuf,
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

struct Sessiond {
    shutting_down: bool,
    loop_signal: calloop::LoopSignal,
    world: World,
    notify_schedule: Schedule,
}

impl Sessiond {
    fn remove_session(&mut self, session_id: session::SessionId) -> Option<Entity> {
        let entity = session::entity(&self.world, session_id)?;
        self.world
            .resource_mut::<session::SessionRegistry>()
            .remove_entity(entity);
        self.world.despawn(entity);
        #[cfg(feature = "consolekit")]
        self.world
            .resource::<consolekit::ConsoleKit>()
            .0
            .remove_session(session_id);
        Some(entity)
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

fn run(runtime_dir: &path::Path, config: config::Config) -> anyhow::Result<()> {
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

    let access_rules = access::AccessRules {
        management: access::Access::new(
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

    let mut world = World::new();
    world.init_resource::<session::PendingSessionEnvs>();
    world.init_resource::<session::PendingSessions>();
    world.insert_resource(session::NextSessionId(session::SessionId::from_raw(1)));
    world.init_resource::<session::SessionInfoManagers>();
    world.init_resource::<power::PowerManagers>();
    world.init_resource::<reconcile::NotifyBatch>();
    world.init_resource::<session::SessionRegistry>();
    world.init_resource::<user::UserRegistry>();
    world.insert_resource(access_rules);
    world.insert_resource(cgroup::CgroupManager::new()?);
    world.insert_resource(config::DaemonConfig(config));
    let state_path = runtime_dir.join("state.toml");
    world.insert_resource(state::StateWriter(state_path.clone()));
    world.insert_non_send(event_loop.handle());
    #[cfg(feature = "consolekit")]
    world.insert_resource(consolekit::ConsoleKit(consolekit::Server::start(
        consolekit_power_sender,
    )));

    let mut schedule = Schedule::new(reconcile::Notify);
    schedule.add_systems(vt::refresh_current_vt.before(user::reconcile_user_states));
    schedule.add_systems(
        user::reconcile_user_states
            .before(state::write_state)
            .before(reconcile::notify_session_active_states)
            .before(power::notify_power_control)
            .before(reconcile::send_done),
    );
    schedule.add_systems(
        (
            state::write_state,
            reconcile::notify_session_active_states,
            power::notify_power_control,
            #[cfg(feature = "consolekit")]
            reconcile::sync_consolekit_sessions,
        )
            .before(reconcile::send_done),
    );
    schedule.add_systems(reconcile::send_done);

    let mut sessiond = Sessiond {
        shutting_down: false,
        loop_signal,
        world,
        notify_schedule: schedule,
    };

    for (uid, runtime_dir) in lingering_users {
        user::create(&mut sessiond.world, uid, true);
        user::create_runtime_dir(uid, &runtime_dir);
        if let Err(e) = sessiond
            .world
            .resource::<cgroup::CgroupManager>()
            .ensure_user_cgroup(uid)
        {
            log::error!(
                uid,
                error = e.to_string().as_str();
                "failed to create user cgroup"
            );
        }
    }
    sessiond.world.clear_trackers();

    fs::create_dir_all(runtime_dir)?;
    fs::set_permissions(runtime_dir, fs::Permissions::from_mode(0o755))?;

    if state_path.exists() {
        match state::State::load(&state_path) {
            Ok(saved_state) => {
                sessiond.world.resource_mut::<session::NextSessionId>().0 =
                    session::SessionId::from_raw(saved_state.next_session_id);

                for (raw_id, saved_session) in saved_state.sessions {
                    let Ok(raw_id) = raw_id.parse::<u32>() else {
                        log::warn!(session_id = raw_id.as_str(); "ignoring invalid session ID in state");
                        continue;
                    };
                    let session_id = session::SessionId::from_raw(raw_id);

                    let cgroup = match sessiond
                        .world
                        .resource_mut::<cgroup::CgroupManager>()
                        .restore_session_cgroup(saved_session.uid, session_id)
                    {
                        Ok(cgroup) => cgroup,
                        Err(error) => {
                            log::warn!(session_id = raw_id, error = error.to_string().as_str(); "ignoring session with an unusable cgroup");
                            continue;
                        }
                    };

                    user::create_runtime_dir(
                        saved_session.uid,
                        &sessiond
                            .user_config(&saved_session.username)
                            .runtime_dir(saved_session.uid),
                    );

                    let active_state = if saved_session
                        .vt
                        .and_then(num::NonZeroU32::new)
                        .is_some_and(|vt| vt::current_vt().ok() == Some(vt))
                        && saved_session.remote_host.is_none()
                    {
                        session_core_v1::ActiveState::Active
                    } else {
                        session_core_v1::ActiveState::Online
                    };

                    let entity = session::spawn(
                        &mut sessiond.world,
                        session_id,
                        saved_session.class(),
                        saved_session.uid,
                        saved_session.username.clone(),
                        saved_session.vt,
                        saved_session.leader,
                        saved_session.remote_host.clone(),
                        active_state,
                        None,
                        cgroup,
                    );
                    sessiond
                        .world
                        .entity_mut(entity)
                        .insert(session::Timestamp(saved_session.timestamp()));
                    sessiond
                        .world
                        .entity_mut(entity)
                        .insert(session::SessionEnv(saved_session.env.clone()));
                }
            }
            Err(error) => {
                log::error!(error = error.to_string().as_str(); "failed to load session state");
            }
        }
    }

    let socket_path = runtime_dir.join("sessiond.sock");
    let mut socket = server::Server::bind(&socket_path)?;
    fs::set_permissions(socket_path, fs::Permissions::from_mode(0o666))?;

    socket.add_implementation::<session_management_v1::SessionManagementV1, _>(&mut sessiond, 1);
    socket.add_implementation::<session_management_v2::SessionManagementV2, _>(&mut sessiond, 2);
    socket.add_implementation::<session_core_v1::SessionCoreV1, _>(&mut sessiond, 1);
    socket.add_implementation::<session_power_v1::SessionPowerV1, _>(&mut sessiond, 1);

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
        .insert_source(source, move |_, _, state: &mut Sessiond| {
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
                    let can = power::can_control_power(&state.world, &request.creds);
                    let _ = request.reply.send(consolekit::PowerResponse::Can(can));
                }
            }
        })
        .map_err(|err| anyhow::anyhow!("failed to insert ConsoleKit power source: {err}"))?;

    event_loop
        .handle()
        .insert_source(signals, move |event, (), state| match event.signal() {
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
                        &session::Vt,
                        &session::IsLocal,
                        &mut session_core_v1::ActiveState,
                    ), With<session::Session>>();
                    for (vt, _, mut active_state) in sessions.iter_mut(&mut state.world) {
                        let desired = if vt.0 == current {
                            session_core_v1::ActiveState::Active
                        } else {
                            session_core_v1::ActiveState::Online
                        };
                        if *active_state != desired {
                            *active_state = desired;
                        }
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
                state.loop_signal.stop();
            }
            _ => unreachable!(),
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

    run(&args.runtime_dir, config::Config::load(args.config)?)
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
