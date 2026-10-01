#[path = "../../shared/logging.rs"]
mod logging;

use calloop::generic;
use clap::{Parser, ValueEnum};
use hyprwire::{client, server};
use hyprwire_protocols_sessiond::client::session_core::v1::session_core_v1;
use hyprwire_protocols_sessiond::client::session_core::v1::session_core_v1::{
    session_core_manager, session_core_session,
};
use hyprwire_protocols_sessiond::server::session_backlight::v1::session_backlight_v1;
use hyprwire_protocols_sessiond::server::session_backlight::v1::session_backlight_v1::{
    session_backlight_device, session_backlight_manager,
};
use std::io::{Seek, Write};
use std::os::unix::fs::PermissionsExt;
use std::{env, fs, io, path, process};

#[derive(Parser)]
#[command(version, about = "sessiond backlight daemon")]
struct Cli {
    #[arg(long, value_enum, default_value_t = LogTarget::Stderr)]
    log_target: LogTarget,
}

#[derive(Clone, Copy, ValueEnum)]
enum LogTarget {
    Stderr,
    Syslog,
}

enum BrightnessError {
    InvalidArgs(String),
    Failed(String),
}

struct Device {
    object: session_backlight_device::SessionBacklightDevice,
    pub subsystem: String,
    pub name: String,
    pub brightness: fs::File,
    pub max_brightness: u32,
}

impl Device {
    fn validate_component(kind: &str, value: &str) -> Result<(), BrightnessError> {
        let mut components = path::Path::new(value).components();
        if value.is_empty()
            || !matches!(components.next(), Some(path::Component::Normal(_)))
            || components.next().is_some()
        {
            return Err(BrightnessError::InvalidArgs(format!(
                "invalid brightness {kind} {value}"
            )));
        }

        Ok(())
    }

    fn open(
        session_backlight_device: session_backlight_device::SessionBacklightDevice,
        subsystem: String,
        name: String,
    ) -> Result<Self, BrightnessError> {
        if !matches!(subsystem.as_str(), "backlight" | "leds") {
            return Err(BrightnessError::InvalidArgs(format!(
                "unsupported brightness subsystem {subsystem}"
            )));
        }
        Self::validate_component("device name", &name)?;

        let path = path::Path::new("/sys/class").join(&subsystem).join(&name);

        let max_brightness = fs::read_to_string(path.join("max_brightness"))
            .map_err(|err| {
                BrightnessError::Failed(format!(
                    "failed to read {subsystem}/{name} max_brightness: {err}"
                ))
            })?
            .trim()
            .parse::<u32>()
            .map_err(|err| {
                BrightnessError::Failed(format!(
                    "failed to parse {subsystem}/{name} max_brightness: {err}"
                ))
            })?;

        let brightness = fs::OpenOptions::new()
            .write(true)
            .open(path.join("brightness"))
            .map_err(|err| {
                BrightnessError::Failed(format!(
                    "failed to open {subsystem}/{name} brightness: {err}"
                ))
            })?;

        log::debug!("opened {subsystem}/{name} with max_brightness={max_brightness}");

        Ok(Self {
            subsystem,
            name,
            brightness,
            max_brightness,
            object: session_backlight_device,
        })
    }

    fn set(&mut self, brightness: u32) -> anyhow::Result<()> {
        if brightness > self.max_brightness {
            anyhow::bail!(
                "brightness {brightness} exceeds max_brightness {}",
                self.max_brightness
            );
        }

        self.brightness.seek(io::SeekFrom::Start(0))?;
        self.brightness
            .write_all(brightness.to_string().as_bytes())?;
        self.brightness.write_all(b"\n")?;
        self.brightness.flush()?;

        Ok(())
    }
}

struct Session {
    id: u32,
    object: session_core_session::SessionCoreSession,
    local: bool,
    uid: u32,
    active: session_core_v1::ActiveState,
    done: bool,
}

impl Session {
    const fn new(id: u32, object: session_core_session::SessionCoreSession) -> Self {
        Self {
            id,
            object,
            local: false,
            uid: 0,
            done: false,
            active: session_core_v1::ActiveState::Opening,
        }
    }
}

#[derive(Clone, Copy)]
struct SessionAccess {
    id: u32,
    local: bool,
    uid: u32,
    active: session_core_v1::ActiveState,
    done: bool,
}

impl From<&Session> for SessionAccess {
    fn from(session: &Session) -> Self {
        Self {
            id: session.id,
            local: session.local,
            uid: session.uid,
            active: session.active,
            done: session.done,
        }
    }
}

struct SessionAccessState {
    sessions: Vec<SessionAccess>,
}

impl SessionAccessState {
    const fn new() -> Self {
        Self {
            sessions: Vec::new(),
        }
    }

    fn update(&mut self, session: &Session) {
        let access = SessionAccess::from(session);

        if let Some(existing) = self
            .sessions
            .iter_mut()
            .find(|existing| existing.id == session.id)
        {
            *existing = access;
        } else {
            self.sessions.push(access);
        }
    }

    fn remove(&mut self, session_id: u32) {
        self.sessions.retain(|session| session.id != session_id);
    }

    fn has_active_local_session(&self, uid: u32) -> bool {
        self.sessions.iter().any(|session| {
            session.done
                && session.local
                && session.uid == uid
                && matches!(session.active, session_core_v1::ActiveState::Active)
        })
    }
}

struct SessiondBacklight {
    session_core_manager: Option<session_core_manager::SessionCoreManager>,
    sessions: Vec<Session>,
    access: SessionAccessState,
    devices: Vec<Device>,
}

impl hyprwire::Dispatch<session_core_manager::SessionCoreManager> for SessiondBacklight {
    fn event(
        &mut self,
        object: &session_core_manager::SessionCoreManager,
        event: <session_core_manager::SessionCoreManager as hyprwire::Object>::Event<'_>,
    ) {
        if let session_core_manager::Event::SessionAdded { session_id } = event {
            if let Some(session) = object.send_get_session_by_id::<Self>(session_id) {
                self.sessions.push(Session::new(session_id, session));
            } else {
                log::error!("failed to request session {session_id}");
            }
        }
    }
}

impl hyprwire::Dispatch<session_core_session::SessionCoreSession> for SessiondBacklight {
    fn event(
        &mut self,
        object: &session_core_session::SessionCoreSession,
        event: <session_core_session::SessionCoreSession as hyprwire::Object>::Event<'_>,
    ) {
        let Some(session) = self
            .sessions
            .iter_mut()
            .find(|session| session.object == *object)
        else {
            return;
        };

        match event {
            session_core_session::Event::Active { state } => session.active = state,
            session_core_session::Event::Uid { uid } => session.uid = uid,
            session_core_session::Event::IsLocal { is_local } => session.local = is_local != 0,
            session_core_session::Event::Done => session.done = true,
            session_core_session::Event::Destroyed => {
                let session_id = session.id;
                self.sessions.retain(|session| session.id != session_id);
                self.access.remove(session_id);
                return;
            }
            _ => {}
        }

        self.access.update(session);
    }
}

impl hyprwire::Handler<session_backlight_v1::SessionBacklightV1> for SessiondBacklight {}

impl hyprwire::Dispatch<session_backlight_manager::SessionBacklightManager> for SessiondBacklight {
    fn event(
        &mut self,
        object: &session_backlight_manager::SessionBacklightManager,
        event: <session_backlight_manager::SessionBacklightManager as hyprwire::Object>::Event<'_>,
    ) {
        if let session_backlight_manager::Event::GetDevice {
            session_backlight_device,
            subsystem,
            name,
        } = event
        {
            match Device::open(session_backlight_device, subsystem, name) {
                Ok(device) => self.devices.push(device),
                Err(error) => {
                    let (code, message) = match error {
                        BrightnessError::InvalidArgs(message) => {
                            (session_backlight_v1::ErrorCode::InvalidArgs, message)
                        }
                        BrightnessError::Failed(message) => {
                            (session_backlight_v1::ErrorCode::Io, message)
                        }
                    };
                    object.send_error(code, message);
                }
            }
        }
    }
}

impl hyprwire::Dispatch<session_backlight_device::SessionBacklightDevice> for SessiondBacklight {
    fn event(
        &mut self,
        object: &session_backlight_device::SessionBacklightDevice,
        event: <session_backlight_device::SessionBacklightDevice as hyprwire::Object>::Event<'_>,
    ) {
        match event {
            session_backlight_device::Event::SetBrightness { brightness } => {
                let client = object.client();
                let creds = client.creds();

                let Some(device) = self
                    .devices
                    .iter_mut()
                    .find(|device| device.object == *object)
                else {
                    return;
                };

                log::debug!(
                    "SetBrightness called: uid={}, pid={}, subsystem={}, name={}, brightness={brightness}",
                    creds.uid.as_raw(),
                    creds.pid.as_raw_pid(),
                    device.subsystem,
                    device.name,
                );

                if !self.access.has_active_local_session(creds.uid.as_raw()) && !creds.uid.is_root()
                {
                    object.send_error(
                        session_backlight_v1::ErrorCode::PermissionDenied,
                        format!(
                            "caller uid {} has no active local session",
                            creds.uid.as_raw()
                        ),
                    );
                    return;
                }

                match device.set(brightness) {
                    Ok(()) => object.send_done(),
                    Err(message) => {
                        object.send_error(session_backlight_v1::ErrorCode::Io, message.to_string());
                    }
                }
            }
            session_backlight_device::Event::Destroy => {
                self.devices.retain(|device| device.object != *object);
            }
            _ => {}
        }
    }
}

fn main() -> process::ExitCode {
    let cli = Cli::parse();
    if let Err(error) = init_logging(cli.log_target) {
        eprintln!("failed to initialize logging: {error:#}");
        return process::ExitCode::FAILURE;
    }

    let status = match run() {
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
                .filter(Some("sessiond_backlight"), log_level)
                .try_init()?;
        }
        LogTarget::Syslog => {
            logging::init_syslog(
                syslog::Facility::LOG_DAEMON,
                log_level,
                "sessiond-backlight",
            )?;
        }
    }
    Ok(())
}

fn run() -> anyhow::Result<()> {
    let mut event_loop = calloop::EventLoop::try_new()?;

    let mut sessiond_backlight = SessiondBacklight {
        session_core_manager: None,
        sessions: Vec::new(),
        access: SessionAccessState::new(),
        devices: Vec::new(),
    };

    let sessiond_runtime_dir: path::PathBuf = env::var_os("SESSIOND_RUNTIME_DIR")
        .map_or_else(|| path::PathBuf::from("/run/sessiond"), path::PathBuf::from);
    let backlight_socket_path = sessiond_runtime_dir.join("backlight.sock");

    let mut server = server::Server::bind(&backlight_socket_path)?;
    fs::set_permissions(backlight_socket_path, fs::Permissions::from_mode(0o666))?;

    server.add_implementation::<session_backlight_v1::SessionBacklightV1, _>(
        &mut sessiond_backlight,
        1,
    );

    let mut client = client::Client::connect(sessiond_runtime_dir.join("sessiond.sock"))?;
    let eq = client.new_event_queue();

    client.add_implementation::<session_core_v1::SessionCoreV1>();

    eq.wait_for_handshake(&mut sessiond_backlight)?;

    sessiond_backlight.session_core_manager =
        Some(client.bind::<session_core_manager::SessionCoreManager, _>(
            &eq,
            &mut sessiond_backlight,
            1,
        )?);

    let fd_wrapper =
        unsafe { generic::FdWrapper::new(client.extract_loop_fd().try_clone_to_owned()?) };
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
            _ = eq.dispatch_events(state, false);
            Ok(calloop::PostAction::Continue)
        })?;

    let fd_wrapper =
        unsafe { generic::FdWrapper::new(server.extract_loop_fd().try_clone_to_owned()?) };
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
            _ = server.dispatch_events(state, false);
            Ok(calloop::PostAction::Continue)
        })?;

    event_loop.run(None, &mut sessiond_backlight, |_| {})?;

    Ok(())
}
