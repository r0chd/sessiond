use crate::{ecs, session, session_core_v1};
use std::collections::HashMap;
use std::sync;
use std::sync::mpsc;
use std::thread;
use std::time;
use zbus::blocking;
use zbus::blocking::fdo::DBusProxy;
use zbus::message;
use zbus::names;
use zbus::zvariant;
use zbus::{fdo, interface};

const BUS_NAME: &str = "org.freedesktop.ConsoleKit";
const MANAGER_PATH: &str = "/org/freedesktop/ConsoleKit/Manager";
#[derive(Clone)]
pub struct SessionInfo {
    pub id: session::SessionId,
    pub uid: u32,
    pub seat: String,
    pub timestamp: time::SystemTime,
    pub is_local: bool,
    pub active: session_core_v1::ActiveState,
}

impl SessionInfo {
    pub fn from_world(
        world: &ecs::World,
        entity: ecs::Entity,
        id: session::SessionId,
    ) -> Option<Self> {
        Some(Self {
            id,
            uid: world.get::<ecs::Uid>(entity)?.0,
            seat: world
                .get::<ecs::SeatName>(entity)
                .map_or_else(|| "seat0".to_string(), |seat| seat.0.clone()),
            timestamp: world.get::<ecs::Timestamp>(entity)?.0,
            is_local: world
                .get::<ecs::IsLocal>(entity)
                .is_none_or(|local| local.0),
            active: world.get::<ecs::ActiveState>(entity)?.0,
        })
    }
}

pub struct Server {
    sender: mpsc::Sender<Command>,
}

impl Server {
    pub fn start() -> Self {
        let (sender, receiver) = mpsc::channel();
        if let Err(err) = thread::Builder::new()
            .name("sessiond-consolekit".to_string())
            .spawn(move || run(receiver))
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

#[derive(Default)]
struct State {
    sessions: HashMap<u32, SessionInfo>,
}

type SharedState = sync::Arc<sync::RwLock<State>>;

struct Manager {
    state: SharedState,
}

#[interface(name = "org.freedesktop.ConsoleKit.Manager")]
impl Manager {
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

    #[zbus(name = "IsActive")]
    fn is_active(&self) -> fdo::Result<bool> {
        self.with_session(|session| session.active == session_core_v1::ActiveState::Active)
    }

    #[zbus(name = "IsLocal")]
    fn is_local(&self) -> fdo::Result<bool> {
        self.with_session(|session| session.is_local)
    }

    #[zbus(name = "GetSeatId")]
    fn get_seat_id(&self) -> fdo::Result<zvariant::OwnedObjectPath> {
        self.with_session(|session| {
            let seat = if session.seat.is_empty() {
                "seat0"
            } else {
                session.seat.as_str()
            };
            object_path(&format!("/org/freedesktop/ConsoleKit/Seat/{seat}"))
        })?
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

#[allow(clippy::needless_pass_by_value)]
fn run(receiver: mpsc::Receiver<Command>) {
    let state = SharedState::default();
    let mut connection = None;
    let mut warned_unavailable = false;

    loop {
        if connection.is_none() {
            match start_connection(&state) {
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

fn start_connection(state: &SharedState) -> zbus::Result<blocking::Connection> {
    zbus::blocking::connection::Builder::system()?
        .serve_at(
            MANAGER_PATH,
            Manager {
                state: state.clone(),
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
    object_path(&format!(
        "/org/freedesktop/ConsoleKit/Session/{}",
        session_id.as_raw()
    ))
    .unwrap_or_else(|_| {
        zvariant::ObjectPath::from_static_str_unchecked("/org/freedesktop/ConsoleKit/Session/0")
            .into()
    })
}

fn object_path(path: &str) -> fdo::Result<zvariant::OwnedObjectPath> {
    zvariant::ObjectPath::try_from(path)
        .map(Into::into)
        .map_err(|err| fdo::Error::Failed(format!("invalid object path: {err}")))
}

fn session_id_for_pid(pid: u32) -> fdo::Result<u32> {
    let cgroup = std::fs::read_to_string(format!("/proc/{pid}/cgroup"))
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

fn timestamp_string(timestamp: time::SystemTime) -> String {
    timestamp
        .duration_since(time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string()
}
