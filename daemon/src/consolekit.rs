use crate::{power, session, session_core_v1};
use power::{PowerAction, PowerError};
use calloop::channel;
use rustix::process;
use std::collections::HashMap;
use std::fmt::Write as _;
use std::io::Write as _;
use std::sync::mpsc;
use std::{fs, io, sync, thread, time};
use zbus::blocking::connection;
use zbus::blocking::fdo::DBusProxy;
use zbus::{blocking, fdo, interface, message, names, zvariant};

const BUS_NAME: &str = "org.freedesktop.ConsoleKit";
const MANAGER_PATH: &str = "/org/freedesktop/ConsoleKit/Manager";
const DATABASE_DIR: &str = "/var/run/ConsoleKit";
const DATABASE_PATH: &str = "/var/run/ConsoleKit/database";
const DATABASE_TMP_PATH: &str = "/var/run/ConsoleKit/database~";

#[derive(Clone)]
pub struct SessionInfo {
    pub id: session::SessionId,
    pub uid: u32,
    pub timestamp: time::SystemTime,
    pub is_local: bool,
    pub active: session_core_v1::ActiveState,
}

pub struct Server {
    sender: mpsc::Sender<Command>,
}

impl Server {
    pub fn start(power_sender: channel::Sender<PowerRequest>) -> Self {
        let (sender, receiver) = mpsc::channel();
        if let Err(err) = thread::Builder::new()
            .name("sessiond-consolekit".to_string())
            .spawn(move || run(&receiver, &power_sender))
        {
            log::error!(error = err.to_string().as_str(); "failed to spawn ConsoleKit thread");
        }

        Self { sender }
    }

    pub fn upsert_session(&self, session: SessionInfo) {
        if let Err(err) = self.sender.send(Command::Upsert(session)) {
            log::debug!(error = err.to_string().as_str(); "failed to update ConsoleKit session");
        }
    }

    pub fn remove_session(&self, session_id: session::SessionId) {
        if let Err(err) = self.sender.send(Command::Remove(session_id.as_raw())) {
            log::debug!(error = err.to_string().as_str(); "failed to remove ConsoleKit session");
        }
    }
}

enum Command {
    Upsert(SessionInfo),
    Remove(u32),
}

pub struct PowerRequest {
    pub kind: PowerRequestKind,
    pub creds: hyprwire::Creds,
    pub reply: mpsc::Sender<PowerResponse>,
}

pub enum PowerRequestKind {
    Execute(PowerAction),
    Can(PowerAction),
}

pub enum PowerResponse {
    Done(Result<(), PowerError>),
    Can(bool),
}

#[derive(Default)]
struct State {
    sessions: HashMap<u32, SessionInfo>,
}

type SharedState = sync::Arc<sync::RwLock<State>>;

struct Manager {
    state: SharedState,
    power_sender: channel::Sender<PowerRequest>,
}

#[allow(clippy::needless_pass_by_value)]
#[interface(name = "org.freedesktop.ConsoleKit.Manager")]
impl Manager {
    #[zbus(name = "Restart")]
    fn restart(&self, #[zbus(header)] header: message::Header<'_>) -> fdo::Result<()> {
        self.execute_power(PowerAction::Reboot, &header)
    }

    #[zbus(name = "Stop")]
    fn stop(&self, #[zbus(header)] header: message::Header<'_>) -> fdo::Result<()> {
        self.execute_power(PowerAction::Poweroff, &header)
    }

    #[zbus(name = "Suspend")]
    fn suspend(
        &self,
        #[allow(unused_variables)] policykit_interactivity: bool,
        #[zbus(header)] header: message::Header<'_>,
    ) -> fdo::Result<()> {
        self.execute_power(PowerAction::Suspend, &header)
    }

    #[zbus(name = "Hibernate")]
    fn hibernate(
        &self,
        #[allow(unused_variables)] policykit_interactivity: bool,
        #[zbus(header)] header: message::Header<'_>,
    ) -> fdo::Result<()> {
        self.execute_power(PowerAction::Hibernate, &header)
    }

    #[zbus(name = "CanRestart")]
    fn can_restart(&self, #[zbus(header)] header: message::Header<'_>) -> fdo::Result<bool> {
        self.can_power(PowerAction::Reboot, &header)
    }

    #[zbus(name = "CanStop")]
    fn can_stop(&self, #[zbus(header)] header: message::Header<'_>) -> fdo::Result<bool> {
        self.can_power(PowerAction::Poweroff, &header)
    }

    #[zbus(name = "CanSuspend")]
    fn can_suspend(&self, #[zbus(header)] header: message::Header<'_>) -> fdo::Result<String> {
        self.can_power_string(PowerAction::Suspend, &header)
    }

    #[zbus(name = "CanHibernate")]
    fn can_hibernate(&self, #[zbus(header)] header: message::Header<'_>) -> fdo::Result<String> {
        self.can_power_string(PowerAction::Hibernate, &header)
    }

    #[zbus(name = "GetSessionForUnixProcess")]
    fn get_session_for_unix_process(&self, pid: u32) -> fdo::Result<zvariant::OwnedObjectPath> {
        let session_id = session_id_for_pid(pid)?;
        self.with_session(session_id, |session| session_path(session.id))
    }

    #[zbus(name = "GetCurrentSession")]
    #[allow(clippy::needless_pass_by_value)]
    fn get_current_session(
        &self,
        #[zbus(header)] header: message::Header<'_>,
    ) -> fdo::Result<zvariant::OwnedObjectPath> {
        let sender = header
            .sender()
            .ok_or_else(|| fdo::Error::Failed("caller has no D-Bus sender".to_string()))?;
        let connection = blocking::Connection::system()
            .map_err(|err| fdo::Error::Failed(format!("failed to connect to system bus: {err}")))?;
        let proxy = DBusProxy::new(&connection)
            .map_err(|err| fdo::Error::Failed(format!("failed to create D-Bus proxy: {err}")))?;
        let pid = proxy
            .get_connection_unix_process_id(names::BusName::from(sender.clone()))
            .map_err(|err| fdo::Error::Failed(format!("failed to resolve caller pid: {err}")))?;
        let session_id = session_id_for_pid(pid)?;
        self.with_session(session_id, |session| session_path(session.id))
    }

    #[zbus(name = "GetSessions")]
    fn get_sessions(&self) -> fdo::Result<Vec<zvariant::OwnedObjectPath>> {
        let state = self
            .state
            .read()
            .map_err(|_| fdo::Error::Failed("ConsoleKit state lock poisoned".to_string()))?;
        Ok(state
            .sessions
            .values()
            .map(|session| session_path(session.id))
            .collect())
    }
}

impl Manager {
    fn execute_power(
        &self,
        action: PowerAction,
        header: &message::Header<'_>,
    ) -> fdo::Result<()> {
        match self.send_power_request(PowerRequestKind::Execute(action), header)? {
            PowerResponse::Done(Ok(())) => Ok(()),
            PowerResponse::Done(Err(PowerError::ActiveSessions(message))) => {
                Err(fdo::Error::AccessDenied(message))
            }
            PowerResponse::Done(Err(PowerError::Io(message))) => {
                Err(fdo::Error::Failed(message))
            }
            PowerResponse::Can(_) => Err(fdo::Error::Failed(
                "unexpected ConsoleKit power response".to_string(),
            )),
        }
    }

    fn can_power(
        &self,
        action: PowerAction,
        header: &message::Header<'_>,
    ) -> fdo::Result<bool> {
        match self.send_power_request(PowerRequestKind::Can(action), header)? {
            PowerResponse::Can(can) => Ok(can),
            PowerResponse::Done(_) => Err(fdo::Error::Failed(
                "unexpected ConsoleKit power response".to_string(),
            )),
        }
    }

    fn can_power_string(
        &self,
        action: PowerAction,
        header: &message::Header<'_>,
    ) -> fdo::Result<String> {
        self.can_power(action, header)
            .map(|can| if can { "yes" } else { "no" }.to_string())
    }

    fn send_power_request(
        &self,
        kind: PowerRequestKind,
        header: &message::Header<'_>,
    ) -> fdo::Result<PowerResponse> {
        let creds = caller_creds(header)?;
        let (reply, receiver) = mpsc::channel();
        self.power_sender
            .send(PowerRequest { kind, creds, reply })
            .map_err(|_| fdo::Error::Failed("ConsoleKit power handler unavailable".to_string()))?;
        receiver
            .recv()
            .map_err(|_| fdo::Error::Failed("ConsoleKit power handler closed".to_string()))
    }

    fn with_session<T>(
        &self,
        session_id: u32,
        f: impl FnOnce(&SessionInfo) -> T,
    ) -> fdo::Result<T> {
        let session = {
            let state = self
                .state
                .read()
                .map_err(|_| fdo::Error::Failed("ConsoleKit state lock poisoned".to_string()))?;
            state
                .sessions
                .get(&session_id)
                .cloned()
                .ok_or_else(|| fdo::Error::Failed("unknown ConsoleKit session".to_string()))?
        };

        Ok(f(&session))
    }
}

struct Session {
    id: u32,
    state: SharedState,
}

#[interface(name = "org.freedesktop.ConsoleKit.Session")]
impl Session {
    #[zbus(name = "GetUnixUser")]
    fn get_unix_user(&self) -> fdo::Result<u32> {
        self.with_session(|session| session.uid)
    }

    #[zbus(name = "GetUser")]
    fn get_user(&self) -> fdo::Result<u32> {
        self.get_unix_user()
    }

    #[zbus(name = "IsActive")]
    fn is_active(&self) -> fdo::Result<bool> {
        self.with_session(is_active)
    }

    #[zbus(name = "IsLocal")]
    fn is_local(&self) -> fdo::Result<bool> {
        self.with_session(|session| session.is_local)
    }

    #[zbus(name = "GetId")]
    fn get_id(&self) -> fdo::Result<String> {
        self.with_session(|session| session.id.as_raw().to_string())
    }

    #[zbus(name = "GetLoginSessionId")]
    fn get_login_session_id(&self) -> fdo::Result<String> {
        self.with_session(|session| session.id.as_raw().to_string())
    }

    #[zbus(name = "GetCreationTime")]
    fn get_creation_time(&self) -> fdo::Result<String> {
        self.with_session(|session| timestamp_string(session.timestamp))
    }

    #[zbus(property, name = "unix-user")]
    fn unix_user(&self) -> fdo::Result<u32> {
        self.get_unix_user()
    }

    #[zbus(property, name = "is-active")]
    fn active_property(&self) -> fdo::Result<bool> {
        self.is_active()
    }

    #[zbus(property, name = "is-local")]
    fn local_property(&self) -> fdo::Result<bool> {
        self.is_local()
    }
}

impl Session {
    fn with_session<T>(&self, f: impl FnOnce(&SessionInfo) -> T) -> fdo::Result<T> {
        let session = {
            let state = self
                .state
                .read()
                .map_err(|_| fdo::Error::Failed("ConsoleKit state lock poisoned".to_string()))?;
            state
                .sessions
                .get(&self.id)
                .cloned()
                .ok_or_else(|| fdo::Error::Failed("unknown ConsoleKit session".to_string()))?
        };

        Ok(f(&session))
    }
}

fn run(receiver: &mpsc::Receiver<Command>, power_sender: &channel::Sender<PowerRequest>) {
    let state = SharedState::default();
    let mut connection = None;
    let mut warned_unavailable = false;

    if let Err(err) = write_database(&state) {
        log::warn!(error = err.to_string().as_str(); "failed to write ConsoleKit database");
    }

    loop {
        if connection.is_none() {
            match start_connection(&state, power_sender.clone()) {
                Ok(new_connection) => {
                    register_existing_sessions(&new_connection, &state);
                    connection = Some(new_connection);
                    warned_unavailable = false;
                    log::info!("ConsoleKit compatibility layer started");
                }
                Err(err) => {
                    if !warned_unavailable {
                        log::warn!(error = err.to_string().as_str(); "ConsoleKit compatibility waiting for system bus");
                        warned_unavailable = true;
                    }
                }
            }
        }

        match receiver.recv_timeout(time::Duration::from_secs(1)) {
            Ok(command) => apply_command(connection.as_ref(), &state, command),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => return,
        }
    }
}

fn start_connection(
    state: &SharedState,
    power_sender: channel::Sender<PowerRequest>,
) -> zbus::Result<blocking::Connection> {
    connection::Builder::system()?
        .serve_at(
            MANAGER_PATH,
            Manager {
                state: state.clone(),
                power_sender,
            },
        )?
        .name(BUS_NAME)?
        .build()
}

fn register_existing_sessions(connection: &blocking::Connection, state: &SharedState) {
    let sessions = state
        .read()
        .map(|state| state.sessions.keys().copied().collect::<Vec<_>>())
        .unwrap_or_default();

    for id in sessions {
        register_session(connection, state, id);
    }
}

fn apply_command(connection: Option<&blocking::Connection>, state: &SharedState, command: Command) {
    match command {
        Command::Upsert(session) => {
            let id = session.id.as_raw();
            let mut should_register = false;
            if let Ok(mut state) = state.write() {
                should_register = !state.sessions.contains_key(&id);
                state.sessions.insert(id, session);
            }

            if should_register && let Some(connection) = connection {
                register_session(connection, state, id);
            }

            if let Err(err) = write_database(state) {
                log::warn!(error = err.to_string().as_str(); "failed to write ConsoleKit database");
            }
        }
        Command::Remove(id) => {
            if let Ok(mut state) = state.write() {
                state.sessions.remove(&id);
            }

            if let Some(connection) = connection {
                let path = session_path(session::SessionId::from_raw(id));
                if let Err(err) = connection.object_server().remove::<Session, _>(path) {
                    log::debug!(session_id = id, error = err.to_string().as_str(); "failed to unregister ConsoleKit session object");
                }
            }

            if let Err(err) = write_database(state) {
                log::warn!(error = err.to_string().as_str(); "failed to write ConsoleKit database");
            }
        }
    }
}

fn register_session(connection: &blocking::Connection, state: &SharedState, id: u32) {
    if let Err(err) = connection.object_server().at(
        session_path(session::SessionId::from_raw(id)),
        Session {
            id,
            state: state.clone(),
        },
    ) {
        log::debug!(session_id = id, error = err.to_string().as_str(); "failed to register ConsoleKit session object");
    }
}

fn session_path(session_id: session::SessionId) -> zvariant::OwnedObjectPath {
    object_path(&session_path_string(session_id)).unwrap_or_else(|_| {
        zvariant::ObjectPath::from_static_str_unchecked("/org/freedesktop/ConsoleKit/Session/0")
            .into()
    })
}

fn session_path_string(session_id: session::SessionId) -> String {
    format!(
        "/org/freedesktop/ConsoleKit/Session/{}",
        session_id.as_raw()
    )
}

fn object_path(path: &str) -> fdo::Result<zvariant::OwnedObjectPath> {
    zvariant::ObjectPath::try_from(path)
        .map(Into::into)
        .map_err(|err| fdo::Error::Failed(format!("invalid object path: {err}")))
}

fn caller_creds(header: &message::Header<'_>) -> fdo::Result<hyprwire::Creds> {
    let sender = header
        .sender()
        .ok_or_else(|| fdo::Error::Failed("caller has no D-Bus sender".to_string()))?;
    let connection = blocking::Connection::system()
        .map_err(|err| fdo::Error::Failed(format!("failed to connect to system bus: {err}")))?;
    let proxy = DBusProxy::new(&connection)
        .map_err(|err| fdo::Error::Failed(format!("failed to create D-Bus proxy: {err}")))?;
    let credentials = proxy
        .get_connection_credentials(names::BusName::from(sender.clone()))
        .map_err(|err| {
            fdo::Error::Failed(format!("failed to resolve caller credentials: {err}"))
        })?;

    let raw_pid = credentials
        .process_id()
        .ok_or_else(|| fdo::Error::Failed("caller credentials have no pid".to_string()))?;
    let raw_uid = credentials
        .unix_user_id()
        .ok_or_else(|| fdo::Error::Failed("caller credentials have no uid".to_string()))?;
    let raw_gid = process_gid(raw_pid).or_else(|| {
        credentials
            .unix_group_ids()
            .and_then(|groups| groups.first().copied())
    });

    let pid = i32::try_from(raw_pid)
        .ok()
        .and_then(process::Pid::from_raw)
        .ok_or_else(|| fdo::Error::Failed(format!("invalid caller pid: {raw_pid}")))?;
    let gid = raw_gid
        .map(process::Gid::from_raw)
        .ok_or_else(|| fdo::Error::Failed("caller credentials have no gid".to_string()))?;

    Ok(hyprwire::Creds {
        pid,
        uid: process::Uid::from_raw(raw_uid),
        gid,
    })
}

fn process_gid(pid: u32) -> Option<u32> {
    let status = fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
    status.lines().find_map(|line| {
        let gids = line.strip_prefix("Gid:")?;
        gids.split_whitespace().nth(1)?.parse().ok()
    })
}

fn session_id_for_pid(pid: u32) -> fdo::Result<u32> {
    let cgroup = fs::read_to_string(format!("/proc/{pid}/cgroup"))
        .map_err(|err| fdo::Error::Failed(format!("could not read process cgroup: {err}")))?;

    cgroup
        .lines()
        .filter_map(|line| line.rsplit_once(':').map(|(_, path)| path))
        .find_map(|path| {
            path.split('/').find_map(|component| {
                component
                    .strip_prefix("session-")
                    .and_then(|id| id.parse::<u32>().ok())
            })
        })
        .ok_or_else(|| fdo::Error::Failed("no ConsoleKit session for pid".to_string()))
}

pub fn timestamp_string(timestamp: time::SystemTime) -> String {
    timestamp
        .duration_since(time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string()
}

fn write_database(state: &SharedState) -> io::Result<()> {
    let sessions = state
        .read()
        .map(|state| state.sessions.values().cloned().collect::<Vec<_>>())
        .unwrap_or_default();

    fs::create_dir_all(DATABASE_DIR)?;

    let mut database = String::new();
    append_database(&mut database, &sessions);

    let mut file = fs::File::create(DATABASE_TMP_PATH)?;
    file.write_all(database.as_bytes())?;
    file.sync_all()?;
    drop(file);

    fs::rename(DATABASE_TMP_PATH, DATABASE_PATH)?;

    Ok(())
}

pub fn append_database(database: &mut String, sessions: &[SessionInfo]) {
    for session in sessions {
        append_session_group(database, session, &session_path_string(session.id));
    }

    append_user_groups(database, sessions);
}

fn append_session_group(database: &mut String, session: &SessionInfo, id: &str) {
    let _ = writeln!(database, "[Session {id}]");
    let _ = writeln!(database, "uid={}", session.uid);
    let _ = writeln!(database, "type=unspecified");
    let _ = writeln!(database, "display_device=");
    let _ = writeln!(database, "remote_host_name=");
    let _ = writeln!(database, "is_active={}", key_file_bool(is_active(session)));
    let _ = writeln!(database, "is_local={}", key_file_bool(session.is_local));
    let _ = writeln!(database, "XDG_RUNTIME_DIR=/run/user/{}", session.uid);
    let _ = writeln!(
        database,
        "creation_time={}",
        key_file_escape(&timestamp_string(session.timestamp))
    );
    let _ = writeln!(database);
}

fn append_user_groups(database: &mut String, sessions: &[SessionInfo]) {
    let mut uids = sessions
        .iter()
        .map(|session| session.uid)
        .collect::<Vec<_>>();
    uids.sort_unstable();
    uids.dedup();

    for uid in uids {
        let user_sessions = sessions
            .iter()
            .filter(|session| session.uid == uid)
            .collect::<Vec<_>>();
        let session_paths = user_sessions
            .iter()
            .map(|session| session_path_string(session.id))
            .collect::<Vec<_>>();
        let is_local = user_sessions.iter().any(|session| session.is_local);

        let _ = writeln!(database, "[User {uid}]");
        let _ = writeln!(database, "is_local={}", key_file_bool(is_local));
        let _ = writeln!(database, "sessions={}", key_file_list(session_paths.iter()));
        let _ = writeln!(database);
    }
}

pub const fn is_active(session: &SessionInfo) -> bool {
    matches!(
        session.active,
        session_core_v1::ActiveState::Active | session_core_v1::ActiveState::Online
    )
}

pub const fn key_file_bool(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}

pub fn key_file_list<'a>(values: impl Iterator<Item = &'a String>) -> String {
    values
        .map(|value| key_file_escape(value))
        .collect::<Vec<_>>()
        .join(";")
}

pub fn key_file_escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            ';' => escaped.push_str("\\;"),
            _ => escaped.push(ch),
        }
    }
    escaped
}
