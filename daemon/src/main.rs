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
mod session_inhibit_v1 {
    hyprwire::include_protocol!("session_inhibit_v1");
    pub use server::*;
    pub use spec::*;
}

mod cgroup;
mod config;
#[cfg(feature = "consolekit")]
mod consolekit;
mod inhibit;
mod power;
mod reconcile;
mod session;
mod state;
mod terminal;
mod user;
mod vt;

use bevy_ecs::message;
use bevy_ecs::prelude::*;
#[cfg(feature = "consolekit")]
use calloop::channel;
use calloop::{generic, signals};
use clap::{Parser, ValueEnum};
use hyprwire::server;
use rustix::net;
use session_core_v1::session_core_user;
use std::os::fd::AsRawFd;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::PermissionsExt;
use std::{env, fs, path};

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

#[derive(Resource, Default, Clone, Copy, PartialEq, Eq)]
pub enum ShutdownState {
    #[default]
    Running,
    Stopping,
}

#[derive(Resource)]
pub struct LoopSignal(pub calloop::LoopSignal);

#[derive(Message)]
struct ShutdownRequested;

struct Sessiond {
    world: World,
    notify_schedule: Schedule,
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

fn handle_shutdown(
    mut requests: MessageReader<ShutdownRequested>,
    mut shutdown_state: ResMut<ShutdownState>,
    loop_signal: Res<LoopSignal>,
) {
    if requests.read().next().is_none() || *shutdown_state == ShutdownState::Stopping {
        return;
    }

    if let Err(error) = notify(b"STOPPING=1\n") {
        log::warn!(error = error.to_string().as_str(); "failed to send shutdown notification");
    }

    *shutdown_state = ShutdownState::Stopping;
    loop_signal.0.stop();
}

fn run(runtime_dir: &path::Path, config: config::Config) -> anyhow::Result<()> {
    let mut event_loop = calloop::EventLoop::try_new()?;
    #[cfg(feature = "consolekit")]
    let (consolekit_power_sender, consolekit_power_channel) = channel::channel();
    let signals = signals::Signals::new(&[
        signals::Signal::SIGUSR1,
        signals::Signal::SIGUSR2,
        signals::Signal::SIGTERM,
        signals::Signal::SIGINT,
    ])?;

    let mut world = World::new();
    message::MessageRegistry::register_message::<session::SessionManagementMessage>(&mut world);
    message::MessageRegistry::register_message::<session::SessionControlMessage>(&mut world);
    message::MessageRegistry::register_message::<session::AddObject>(&mut world);
    message::MessageRegistry::register_message::<inhibit::AddObject>(&mut world);
    message::MessageRegistry::register_message::<session::SessionInfoManagerMessage>(&mut world);
    message::MessageRegistry::register_message::<user::UserControlMessage>(&mut world);
    message::MessageRegistry::register_message::<user::AddObject>(&mut world);
    message::MessageRegistry::register_message::<power::PowerManagerDestroy>(&mut world);
    message::MessageRegistry::register_message::<power::PowerActionRequest>(&mut world);
    #[cfg(feature = "consolekit")]
    message::MessageRegistry::register_message::<power::PowerCanRequest>(&mut world);
    message::MessageRegistry::register_message::<ShutdownRequested>(&mut world);
    world.init_resource::<session::PendingSessions>();
    world.insert_resource(session::NextSessionId(session::SessionId::from_raw(1)));
    world.insert_resource(inhibit::NextInhibitorId(inhibit::InhibitorId::from_raw(1)));
    world.init_resource::<session::SessionInfoManagers>();
    world.init_resource::<ShutdownState>();
    world.init_resource::<reconcile::NotifyBatch>();
    world.init_resource::<session::SessionRegistry>();
    world.init_resource::<user::UserRegistry>();
    world.insert_resource(cgroup::CgroupManager::new()?);
    world.insert_resource(config::DaemonConfig(config));
    world.insert_resource(LoopSignal(event_loop.get_signal()));
    let state_path = runtime_dir.join("state.toml");
    world.insert_resource(state::StateWriter(state_path));
    world.insert_non_send(event_loop.handle());
    #[cfg(feature = "consolekit")]
    world.insert_resource(consolekit::ConsoleKit(consolekit::Server::start(
        consolekit_power_sender,
    )));
    world.add_observer(cgroup::handle_cgroup_empty);

    let mut schedule = Schedule::new(reconcile::Notify);
    schedule.add_systems(user::handle_user_control.before(session::handle_session_control));
    schedule.add_systems(
        session::handle_session_control
            .after(vt::refresh_current_vt)
            .before(user::reconcile_user_states)
            .before(state::write_state)
            .before(reconcile::notify_session_active_states)
            .before(reconcile::send_done),
    );
    schedule.add_systems(
        session::handle_session_management_action
            .before(session::handle_session_control)
            .before(user::ensure_session_users),
    );
    schedule.add_systems(session::handle_add_object.before(reconcile::send_done));
    schedule.add_systems(inhibit::handle_add_object.before(reconcile::send_done));
    schedule.add_systems(session::handle_session_info_manager.before(reconcile::send_done));
    schedule.add_systems(user::handle_add_object.before(reconcile::send_done));
    schedule.add_systems(handle_shutdown.before(reconcile::send_done));
    schedule.add_systems(user::ensure_session_users.before(user::reconcile_user_states));
    schedule.add_systems(vt::refresh_current_vt.before(user::reconcile_user_states));
    schedule.add_systems(
        user::reconcile_user_states
            .before(state::write_state)
            .before(reconcile::notify_session_active_states)
            .before(power::notify_power_control)
            .before(reconcile::send_done),
    );
    schedule.add_systems(
        power::initialize_power_manager_permissions.before(power::notify_power_control),
    );
    schedule.add_systems(power::handle_power_action.before(reconcile::send_done));
    #[cfg(feature = "consolekit")]
    schedule.add_systems(power::handle_power_can.before(reconcile::send_done));
    schedule.add_systems(power::handle_power_manager_control.before(power::notify_power_control));
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
    schedule.add_systems(message::message_update_system.after(reconcile::send_done));

    let mut sessiond = Sessiond {
        world,
        notify_schedule: schedule,
    };

    let mut startup = Schedule::default();
    startup.add_systems((user::initialize_lingering_users, state::load_state));
    startup.add_systems(
        user::ensure_session_users
            .after(user::initialize_lingering_users)
            .after(state::load_state),
    );
    startup.add_systems(vt::refresh_current_vt.after(user::ensure_session_users));
    startup.add_systems(user::reconcile_user_states.after(vt::refresh_current_vt));
    startup.run(&mut sessiond.world);

    sessiond.world.clear_trackers();

    fs::create_dir_all(runtime_dir)?;
    fs::set_permissions(runtime_dir, fs::Permissions::from_mode(0o755))?;

    let socket_path = runtime_dir.join("sessiond.sock");
    let mut socket = server::Server::bind(&socket_path)?;
    fs::set_permissions(socket_path, fs::Permissions::from_mode(0o666))?;

    socket.add_implementation::<session_management_v1::SessionManagementV1, _>(&mut sessiond, 1);
    socket.add_implementation::<session_management_v2::SessionManagementV2, _>(&mut sessiond, 1);
    socket.add_implementation::<session_core_v1::SessionCoreV1, _>(&mut sessiond, 3);
    socket.add_implementation::<session_power_v1::SessionPowerV1, _>(&mut sessiond, 1);
    socket.add_implementation::<session_inhibit_v1::SessionInhibitV1, _>(&mut sessiond, 1);

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
            if *state.world.resource::<ShutdownState>() == ShutdownState::Stopping {
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
                    state.world.write_message(power::PowerActionRequest {
                        action,
                        creds: request.creds,
                        responder: power::PowerActionResponder::ConsoleKit(request.reply),
                    });
                }
                consolekit::PowerRequestKind::Can(action) => {
                    state.world.write_message(power::PowerCanRequest {
                        action,
                        creds: request.creds,
                        responder: request.reply,
                    });
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
                    }
                    signals::Signal::SIGUSR2 => {
                        if let Err(error) = terminal.ack_acquire() {
                            log::error!(
                                error = error.to_string().as_str();
                                "failed to acknowledge VT acquire"
                            );
                        }
                    }
                    _ => {}
                }
            }
            signals::Signal::SIGTERM | signals::Signal::SIGINT => {
                state.world.write_message(ShutdownRequested);
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
