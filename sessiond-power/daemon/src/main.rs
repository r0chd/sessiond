mod config;
#[path = "../../../shared/logging.rs"]
mod logging;

use hyprwire_protocols_sessiond::server::session_power::v1::session_power_v1;

use anyhow::Context;
use calloop::generic;
use clap::{Parser, ValueEnum};
use hyprwire::{client, server};
use hyprwire_protocols_sessiond::client::session_core::v1::session_core_v1;
use hyprwire_protocols_sessiond::client::session_core::v1::session_core_v1::{
    session_core_inhibitor, session_core_manager, session_core_session,
};
use hyprwire_protocols_sessiond::client::session_inhibit::v1::session_inhibit_v1;
use session_power_v1::session_power_manager;
use std::os::unix::fs::PermissionsExt;
use std::{env, fs, path, process};

#[derive(Parser)]
struct Args {
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

struct Session {
    id: u32,
    object: session_core_session::SessionCoreSession,
    uid: Option<u32>,
    class: Option<session_core_v1::SessionClass>,
    complete: bool,
}

struct Inhibitor {
    id: u32,
    object: session_core_inhibitor::SessionCoreInhibitor,
    uid: Option<u32>,
    what: Option<u32>,
    mode: Option<session_core_v1::InhibitorMode>,
    active: Option<bool>,
    complete: bool,
}

#[derive(Clone, Copy)]
enum PowerAction {
    Reboot,
    Poweroff,
    Suspend,
    Hibernate,
    HybridSleep,
}

impl PowerAction {
    const fn name(self) -> &'static str {
        match self {
            Self::Reboot => "reboot",
            Self::Poweroff => "poweroff",
            Self::Suspend => "suspend",
            Self::Hibernate => "hibernate",
            Self::HybridSleep => "hybrid sleep",
        }
    }
}

struct SessiondPower {
    config: config::Config,
    managers: Vec<session_power_manager::SessionPowerManager>,
    session_manager: Option<session_core_manager::SessionCoreManager>,
    sessions: Vec<Session>,
    inhibitors: Vec<Inhibitor>,
    session_manager_done: bool,
    error: Option<String>,
}

impl SessiondPower {
    fn session_state_ready(&self) -> bool {
        self.session_manager_done
            && self
                .sessions
                .iter()
                .all(|session| session.complete && session.uid.is_some() && session.class.is_some())
    }

    fn inhibitor_state_ready(&self) -> bool {
        self.session_manager_done
            && self.inhibitors.iter().all(|inhibitor| {
                inhibitor.complete
                    && inhibitor.uid.is_some()
                    && inhibitor.what.is_some()
                    && inhibitor.mode.is_some()
                    && inhibitor.active.is_some()
            })
    }

    fn denial(&self, action: PowerAction, uid: u32, is_root: bool) -> Option<PowerDenial> {
        if self.error.is_some() || !self.inhibitor_state_ready() {
            return Some(PowerDenial::StateUnavailable);
        }

        let sessions_ready = self.session_state_ready();
        if !sessions_allow(
            uid,
            is_root,
            sessions_ready,
            self.sessions
                .iter()
                .filter(|session| session.class == Some(session_core_v1::SessionClass::User))
                .filter_map(|session| session.uid),
        ) {
            return Some(if sessions_ready {
                PowerDenial::ActiveSessions
            } else {
                PowerDenial::StateUnavailable
            });
        }

        if self.inhibitors.iter().any(|inhibitor| {
            inhibitor_blocks(
                inhibitor.what.unwrap_or_default(),
                inhibitor
                    .mode
                    .unwrap_or(session_core_v1::InhibitorMode::Block),
                inhibitor.uid.unwrap_or_default(),
                inhibitor.active.unwrap_or_default(),
                action,
                uid,
                is_root,
            )
        }) {
            return Some(PowerDenial::Inhibited);
        }

        None
    }

    fn command(&self, action: PowerAction) -> &[String] {
        match action {
            PowerAction::Reboot => &self.config.reboot,
            PowerAction::Poweroff => &self.config.poweroff,
            PowerAction::Suspend => &self.config.suspend,
            PowerAction::Hibernate => &self.config.hibernate,
            PowerAction::HybridSleep => &self.config.hybrid_sleep,
        }
    }

    fn send_capabilities(&self, manager: &session_power_manager::SessionPowerManager) {
        let Some(client) = manager.try_client() else {
            return;
        };
        let uid = client.creds().uid;
        let allowed = |action| {
            !self.command(action).is_empty()
                && self.denial(action, uid.as_raw(), uid.is_root()).is_none()
        };

        manager.send_can_reboot(u32::from(allowed(PowerAction::Reboot)));
        manager.send_can_poweroff(u32::from(allowed(PowerAction::Poweroff)));
        manager.send_can_suspend(u32::from(allowed(PowerAction::Suspend)));
        manager.send_can_hibernate(u32::from(allowed(PowerAction::Hibernate)));
        manager.send_can_hybrid_sleep(u32::from(allowed(PowerAction::HybridSleep)));
    }

    fn notify_capabilities(&mut self) {
        let managers = std::mem::take(&mut self.managers);
        for manager in managers {
            if manager.try_client().is_some() {
                self.send_capabilities(&manager);
                self.managers.push(manager);
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PowerDenial {
    StateUnavailable,
    ActiveSessions,
    Inhibited,
}

fn sessions_allow(
    uid: u32,
    is_root: bool,
    ready: bool,
    mut user_uids: impl Iterator<Item = u32>,
) -> bool {
    is_root || (ready && user_uids.all(|owner| owner == uid))
}

const fn inhibitor_blocks(
    what: u32,
    mode: session_core_v1::InhibitorMode,
    owner_uid: u32,
    active: bool,
    action: PowerAction,
    requester_uid: u32,
    requester_is_root: bool,
) -> bool {
    let action_bit = match action {
        PowerAction::Reboot | PowerAction::Poweroff => session_inhibit_v1::What::Shutdown as u32,
        PowerAction::Suspend | PowerAction::Hibernate | PowerAction::HybridSleep => {
            session_inhibit_v1::What::Sleep as u32
        }
    };
    if !active || what & action_bit == 0 {
        return false;
    }

    match mode {
        session_core_v1::InhibitorMode::BlockWeak => {
            !requester_is_root && owner_uid != requester_uid
        }
        session_core_v1::InhibitorMode::Delay => false,
        _ => true,
    }
}

impl hyprwire::Handler<session_power_v1::SessionPowerV1> for SessiondPower {
    fn bind(&mut self, object: session_power_manager::SessionPowerManager) {
        self.managers.push(object.clone());
        self.send_capabilities(&object);
    }
}

impl hyprwire::Dispatch<session_power_manager::SessionPowerManager> for SessiondPower {
    fn event(
        &mut self,
        object: &session_power_manager::SessionPowerManager,
        event: <session_power_manager::SessionPowerManager as hyprwire::Object>::Event<'_>,
    ) {
        let action = match event {
            session_power_manager::Event::Reboot => Some(PowerAction::Reboot),
            session_power_manager::Event::Poweroff => Some(PowerAction::Poweroff),
            session_power_manager::Event::Suspend => Some(PowerAction::Suspend),
            session_power_manager::Event::Hibernate => Some(PowerAction::Hibernate),
            session_power_manager::Event::HybridSleep => Some(PowerAction::HybridSleep),
            session_power_manager::Event::Destroy => {
                self.managers.retain(|candidate| candidate != object);
                None
            }
            _ => None,
        };

        if let Some(action) = action {
            let command = self.command(action);
            if command.is_empty() {
                object.send_error(
                    session_power_v1::ErrorCode::Io,
                    format!("{} command is not configured", action.name()),
                );
                return;
            }

            let uid = object.client().creds().uid;
            if let Some(denial) = self.denial(action, uid.as_raw(), uid.is_root()) {
                let reason = match denial {
                    PowerDenial::StateUnavailable => "session or inhibitor state is unavailable",
                    PowerDenial::ActiveSessions => "another user has an active session",
                    PowerDenial::Inhibited => "an active inhibitor blocks the operation",
                };
                object.send_error(
                    session_power_v1::ErrorCode::PermissionDenied,
                    format!("cannot {}: {reason}", action.name()),
                );
                return;
            }

            match run_power_command(command, action.name()) {
                Ok(()) => object.send_done(),
                Err(message) => object.send_error(session_power_v1::ErrorCode::Io, message),
            }
        }
    }
}

impl hyprwire::Dispatch<session_core_manager::SessionCoreManager> for SessiondPower {
    fn event(
        &mut self,
        object: &session_core_manager::SessionCoreManager,
        event: <session_core_manager::SessionCoreManager as hyprwire::Object>::Event<'_>,
    ) {
        match event {
            session_core_manager::Event::SessionAdded { session_id } => {
                if !self.sessions.iter().any(|session| session.id == session_id) {
                    if let Some(session) = object.send_get_session_by_id::<Self>(session_id) {
                        self.sessions.push(Session {
                            id: session_id,
                            object: session,
                            uid: None,
                            class: None,
                            complete: false,
                        });
                    } else {
                        self.error = Some(format!("failed to inspect session {session_id}"));
                    }
                }
            }
            session_core_manager::Event::InhibitorAdded { inhibitor_id } => {
                if !self
                    .inhibitors
                    .iter()
                    .any(|inhibitor| inhibitor.id == inhibitor_id)
                {
                    if let Some(inhibitor) = object.send_get_inhibitor::<Self>(inhibitor_id) {
                        self.inhibitors.push(Inhibitor {
                            id: inhibitor_id,
                            object: inhibitor,
                            uid: None,
                            what: None,
                            mode: None,
                            active: None,
                            complete: false,
                        });
                    } else {
                        self.error = Some(format!("failed to inspect inhibitor {inhibitor_id}"));
                    }
                }
            }
            session_core_manager::Event::InhibitorRemoved { inhibitor_id } => {
                self.inhibitors
                    .retain(|inhibitor| inhibitor.id != inhibitor_id);
            }
            session_core_manager::Event::Done => self.session_manager_done = true,
            session_core_manager::Event::Error { message, .. } => self.error = Some(message),
            _ => {}
        }
        self.notify_capabilities();
    }
}

impl hyprwire::Dispatch<session_core_inhibitor::SessionCoreInhibitor> for SessiondPower {
    fn event(
        &mut self,
        object: &session_core_inhibitor::SessionCoreInhibitor,
        event: <session_core_inhibitor::SessionCoreInhibitor as hyprwire::Object>::Event<'_>,
    ) {
        let Some(index) = self
            .inhibitors
            .iter()
            .position(|inhibitor| inhibitor.object == *object)
        else {
            return;
        };

        match event {
            session_core_inhibitor::Event::Uid { uid } => self.inhibitors[index].uid = Some(uid),
            session_core_inhibitor::Event::What { what } => {
                self.inhibitors[index].what = Some(what);
            }
            session_core_inhibitor::Event::Mode { mode } => {
                self.inhibitors[index].mode = Some(mode);
            }
            session_core_inhibitor::Event::Active { active } => {
                self.inhibitors[index].active = Some(active != 0);
            }
            session_core_inhibitor::Event::Done => self.inhibitors[index].complete = true,
            session_core_inhibitor::Event::Destroyed => {
                self.inhibitors.swap_remove(index);
            }
            _ => {}
        }

        self.notify_capabilities();
    }
}

impl hyprwire::Dispatch<session_core_session::SessionCoreSession> for SessiondPower {
    fn event(
        &mut self,
        object: &session_core_session::SessionCoreSession,
        event: <session_core_session::SessionCoreSession as hyprwire::Object>::Event<'_>,
    ) {
        let Some(index) = self
            .sessions
            .iter()
            .position(|session| session.object == *object)
        else {
            return;
        };

        match event {
            session_core_session::Event::Uid { uid } => self.sessions[index].uid = Some(uid),
            session_core_session::Event::Class { class } => {
                self.sessions[index].class = Some(class);
            }
            session_core_session::Event::Done => self.sessions[index].complete = true,
            session_core_session::Event::Destroyed => {
                self.sessions.swap_remove(index);
            }
            _ => {}
        }

        self.notify_capabilities();
    }
}

fn main() -> process::ExitCode {
    let args = Args::parse();
    if let Err(error) = init_logging(args.log_target) {
        eprintln!("failed to initialize logging: {error:#}");
        return process::ExitCode::FAILURE;
    }

    let status = match run(args) {
        Ok(()) => process::ExitCode::SUCCESS,
        Err(error) => {
            log::error!("application failed: {error:#}");
            process::ExitCode::FAILURE
        }
    };
    log::logger().flush();
    status
}

fn init_logging(log_target: LogTarget) -> anyhow::Result<()> {
    let log_level = match env::var("LOG_LEVEL") {
        Ok(log) if log == "trace" => log::LevelFilter::Trace,
        Ok(log) if log == "debug" => log::LevelFilter::Debug,
        Ok(log) if log == "warn" => log::LevelFilter::Warn,
        Ok(log) if log == "error" => log::LevelFilter::Error,
        Ok(log) if log == "off" => log::LevelFilter::Off,
        _ => log::LevelFilter::Info,
    };

    match log_target {
        LogTarget::Stderr => {
            env_logger::Builder::new()
                .filter_level(log_level)
                .try_init()?;
        }
        LogTarget::Syslog => {
            logging::init_syslog(syslog::Facility::LOG_DAEMON, log_level, "sessiond-power")?;
        }
    }
    Ok(())
}

fn run(args: Args) -> anyhow::Result<()> {
    let mut sessiond_power = SessiondPower {
        config: config::Config::load(args.config)?,
        managers: Vec::new(),
        session_manager: None,
        sessions: Vec::new(),
        inhibitors: Vec::new(),
        session_manager_done: false,
        error: None,
    };

    let sessiond_runtime_dir: path::PathBuf = env::var_os("SESSIOND_RUNTIME_DIR")
        .map_or_else(|| path::PathBuf::from("/run/sessiond"), path::PathBuf::from);
    let sessiond_socket_path = sessiond_runtime_dir.join("sessiond.sock");

    let mut session_socket = client::Client::connect(&sessiond_socket_path)
        .with_context(|| format!("failed to connect to {}", sessiond_socket_path.display()))?;
    let session_queue = session_socket.new_event_queue();
    session_socket.add_implementation::<session_core_v1::SessionCoreV1>();
    session_queue.wait_for_handshake(&mut sessiond_power)?;
    let session_manager = session_socket.bind::<session_core_manager::SessionCoreManager, _>(
        &session_queue,
        &mut sessiond_power,
        2,
    )?;
    sessiond_power.session_manager = Some(session_manager);

    while sessiond_power.error.is_none()
        && (!sessiond_power.session_state_ready() || !sessiond_power.inhibitor_state_ready())
    {
        session_queue.dispatch_events(&mut sessiond_power, true)?;
    }
    if let Some(message) = sessiond_power.error.take() {
        anyhow::bail!(message);
    }

    let power_socket_path = sessiond_runtime_dir.join("power.sock");
    let mut power_socket = server::Server::bind(&power_socket_path)?;
    fs::set_permissions(power_socket_path, fs::Permissions::from_mode(0o666))?;
    power_socket.add_implementation::<session_power_v1::SessionPowerV1, _>(&mut sessiond_power, 1);

    let mut event_loop = calloop::EventLoop::<SessiondPower>::try_new()?;
    let loop_handle = event_loop.handle();

    let power_fd = power_socket
        .extract_loop_fd()
        .try_clone_to_owned()
        .context("failed to duplicate power server event fd")?;
    let power_signal = event_loop.get_signal();
    loop_handle.insert_source(
        generic::Generic::new(power_fd, calloop::Interest::READ, calloop::Mode::Level),
        move |_, _, state| {
            if let Err(error) = power_socket.dispatch_events(state, false) {
                state.error = Some(format!("power server failed: {error}"));
                power_signal.stop();
            }
            Ok(calloop::PostAction::Continue)
        },
    )?;

    let session_signal = event_loop.get_signal();
    loop_handle.insert_source(
        generic::Generic::new(
            session_socket.extract_loop_fd().try_clone_to_owned()?,
            calloop::Interest::READ,
            calloop::Mode::Level,
        ),
        move |_, _, state| {
            if let Err(error) = session_queue.dispatch_events(state, false) {
                state.error = Some(format!("sessiond connection failed: {error}"));
            }
            if state.error.is_some() {
                session_signal.stop();
            }
            Ok(calloop::PostAction::Continue)
        },
    )?;

    event_loop.run(None, &mut sessiond_power, |_| {})?;
    if let Some(message) = sessiond_power.error {
        anyhow::bail!(message);
    }

    Ok(())
}

fn run_power_command(command: &[String], action: &str) -> Result<(), String> {
    let Some((program, arguments)) = command.split_first() else {
        return Err(format!("{action} command is not configured"));
    };

    let status = process::Command::new(program)
        .args(arguments)
        .status()
        .map_err(|error| format!("failed to run {action} command {program:?}: {error}"))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!("{action} command {program:?} exited with {status}"))
    }
}
