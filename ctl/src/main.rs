mod session_core_v1 {
    hyprwire::include_protocol!("session_core_v1");
    pub use client::*;
    pub use spec::*;
}

use anyhow::Context;
use clap::{ArgGroup, CommandFactory, Parser, Subcommand, ValueEnum};
use clap_complete::{Shell, generate};
use clap_complete_nushell::Nushell;
use hyprwire::client;
use owo_colors::OwoColorize;
use rustix::process;
use serde_json::json;
use session_core_v1::{
    session_core_manager, session_core_seat, session_core_seat_client, session_core_seat_device,
    session_core_session, session_core_user,
};
use std::fs;

struct Session {
    id: u32,
    object: session_core_session::SessionCoreSession,
    uid: u32,
    username: String,
    class: session_core_v1::SessionClass,
    vt: u32,
    seat: Option<String>,
    leader: u32,
    timestamp: u32,
    active: session_core_v1::ActiveState,
    can_modify: bool,
    done: bool,
}

#[allow(clippy::struct_excessive_bools)]
struct User {
    uid: u32,
    object: session_core_user::SessionCoreUser,
    username: String,
    lingering: bool,
    state: session_core_v1::UserState,
    can_modify: bool,
    not_found: bool,
    done: bool,
}

struct Seat {
    name: String,
    object: session_core_seat::SessionCoreSeat,
    mode: session_core_v1::SeatMode,
    current_vt: u32,
    clients: Vec<u32>,
    can_modify: bool,
    destroyed: bool,
    done: bool,
}

#[allow(clippy::struct_excessive_bools)]
struct SeatClient {
    seat: String,
    pid: u32,
    object: session_core_seat_client::SessionCoreSeatClient,
    session: u32,
    state: session_core_v1::SeatClientState,
    active: bool,
    next: bool,
    uid: Option<u32>,
    gid: Option<u32>,
    devices: Vec<u32>,
    destroyed: bool,
    done: bool,
}

struct SeatDevice {
    client_pid: u32,
    id: u32,
    object: session_core_seat_device::SessionCoreSeatDevice,
    path: String,
    kind: session_core_v1::SeatDeviceKind,
    active: bool,
    destroyed: bool,
    done: bool,
}

struct State {
    sessions: Vec<Session>,
    users: Vec<User>,
    seats: Vec<Seat>,
    seat_clients: Vec<SeatClient>,
    seat_devices: Vec<SeatDevice>,
    done: bool,
    can_control_power: bool,
    error: Option<String>,
}

impl hyprwire::Dispatch<session_core_manager::SessionCoreManager> for State {
    fn event(
        &mut self,
        object: &session_core_manager::SessionCoreManager,
        event: <session_core_manager::SessionCoreManager as hyprwire::Object>::Event<'_>,
    ) {
        match event {
            session_core_manager::Event::SessionAdded { session_id } => {
                self.add_session(object, session_id);
            }
            session_core_manager::Event::Done => {
                self.done = true;
            }
            session_core_manager::Event::CanControlPower { can_control_power } => {
                self.can_control_power = can_control_power != 0;
            }
            session_core_manager::Event::Error { code: _, message } => {
                self.error = Some(message);
                self.done = true;
            }
        }
    }
}

impl State {
    fn add_seat(&mut self, manager: &session_core_manager::SessionCoreManager, name: String) {
        if self.seats.iter().any(|seat| seat.name == name) {
            return;
        }

        let Some(object) = manager.send_get_seat::<Self>(&name) else {
            self.error = Some(format!("failed to create seat object for {name}"));
            self.done = true;
            return;
        };

        self.seats.push(Seat {
            name,
            object,
            mode: session_core_v1::SeatMode::NonVtBound,
            current_vt: 0,
            clients: Vec::new(),
            can_modify: false,
            destroyed: false,
            done: false,
        });
    }

    fn add_seat_client(&mut self, seat_object: &session_core_seat::SessionCoreSeat, pid: u32) {
        let Some(seat) = self.seats.iter().find(|seat| seat.object == *seat_object) else {
            return;
        };

        if self
            .seat_clients
            .iter()
            .any(|client| client.seat == seat.name && client.pid == pid)
        {
            return;
        }

        let Some(object) = seat.object.send_get_client::<Self>(pid) else {
            self.error = Some(format!("failed to create seat client object for pid {pid}"));
            self.done = true;
            return;
        };

        self.seat_clients.push(SeatClient {
            seat: seat.name.clone(),
            pid,
            object,
            session: 0,
            state: session_core_v1::SeatClientState::New,
            active: false,
            next: false,
            uid: None,
            gid: None,
            devices: Vec::new(),
            destroyed: false,
            done: false,
        });
    }

    fn add_seat_device(
        &mut self,
        client_object: &session_core_seat_client::SessionCoreSeatClient,
        device_id: u32,
    ) {
        let Some(client) = self
            .seat_clients
            .iter()
            .find(|client| client.object == *client_object)
        else {
            return;
        };

        if self
            .seat_devices
            .iter()
            .any(|device| device.client_pid == client.pid && device.id == device_id)
        {
            return;
        }

        let Some(object) = client.object.send_get_device::<Self>(device_id) else {
            self.error = Some(format!(
                "failed to create seat device object for pid {} device {device_id}",
                client.pid
            ));
            self.done = true;
            return;
        };

        self.seat_devices.push(SeatDevice {
            client_pid: client.pid,
            id: device_id,
            object,
            path: String::new(),
            kind: session_core_v1::SeatDeviceKind::Unknown,
            active: false,
            destroyed: false,
            done: false,
        });
    }

    fn add_user(&mut self, manager: &session_core_manager::SessionCoreManager, uid: u32) {
        let Some(object) = manager.send_get_user::<Self>(uid) else {
            self.error = Some(format!("failed to create user object for uid {uid}"));
            self.done = true;
            return;
        };

        self.users.push(User {
            uid,
            object,
            username: String::new(),
            lingering: false,
            state: session_core_v1::UserState::Offline,
            can_modify: false,
            not_found: false,
            done: false,
        });
    }

    fn add_session(&mut self, manager: &session_core_manager::SessionCoreManager, session_id: u32) {
        let Some(object) = manager.send_get_session::<Self>(session_id) else {
            self.error = Some(format!(
                "failed to create session object for id {session_id}"
            ));
            self.done = true;
            return;
        };

        self.sessions.push(Session {
            id: session_id,
            object,
            uid: 0,
            username: String::new(),
            class: session_core_v1::SessionClass::User,
            vt: 0,
            seat: None,
            leader: 0,
            timestamp: 0,
            done: false,
            active: session_core_v1::ActiveState::Online,
            can_modify: false,
        });
    }
}

impl hyprwire::Dispatch<session_core_session::SessionCoreSession> for State {
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
            session_core_session::Event::Uid { uid } => session.uid = uid,
            session_core_session::Event::Username { username } => session.username = username,
            session_core_session::Event::Class { class } => session.class = class,
            session_core_session::Event::Vt { vt } => session.vt = vt,
            session_core_session::Event::Seat { seat } => session.seat = Some(seat),
            session_core_session::Event::Leader { leader } => session.leader = leader,
            session_core_session::Event::Timestamp { timestamp } => session.timestamp = timestamp,
            session_core_session::Event::Done => session.done = true,
            session_core_session::Event::Ok => {
                self.done = true;
            }
            session_core_session::Event::Error { code: _, message } => {
                self.error = Some(message);
                self.done = true;
            }
            session_core_session::Event::Active { state } => {
                session.active = state;
            }
            session_core_session::Event::CanModify { can_modify } => {
                session.can_modify = can_modify != 0;
            }
            _ => {}
        }
    }
}

impl hyprwire::Dispatch<session_core_user::SessionCoreUser> for State {
    fn event(
        &mut self,
        object: &session_core_user::SessionCoreUser,
        event: <session_core_user::SessionCoreUser as hyprwire::Object>::Event<'_>,
    ) {
        let Some(user) = self.users.iter_mut().find(|user| user.object == *object) else {
            return;
        };

        match event {
            session_core_user::Event::Uid { uid } => user.uid = uid,
            session_core_user::Event::Username { username } => user.username = username,
            session_core_user::Event::Lingering { lingering } => user.lingering = lingering != 0,
            session_core_user::Event::State { state } => user.state = state,
            session_core_user::Event::CanModify { can_modify } => {
                user.can_modify = can_modify != 0;
            }
            session_core_user::Event::Done | session_core_user::Event::Ok => {
                user.done = true;
            }
            session_core_user::Event::NotFound => {
                user.not_found = true;
                user.done = true;
            }
            session_core_user::Event::Error { code: _, message } => {
                self.error = Some(message);
                user.done = true;
            }
        }
    }
}

impl hyprwire::Dispatch<session_core_seat::SessionCoreSeat> for State {
    fn event(
        &mut self,
        object: &session_core_seat::SessionCoreSeat,
        event: <session_core_seat::SessionCoreSeat as hyprwire::Object>::Event<'_>,
    ) {
        let Some(idx) = self.seats.iter().position(|seat| seat.object == *object) else {
            return;
        };

        match event {
            session_core_seat::Event::Seat { seat } => self.seats[idx].name = seat,
            session_core_seat::Event::Mode { mode } => self.seats[idx].mode = mode,
            session_core_seat::Event::CurrentVt { vt } => self.seats[idx].current_vt = vt,
            session_core_seat::Event::CanModify { can_modify } => {
                self.seats[idx].can_modify = can_modify != 0;
            }
            session_core_seat::Event::ClientAdded { pid } => {
                if !self.seats[idx].clients.contains(&pid) {
                    self.seats[idx].clients.push(pid);
                }
                let object = object.clone();
                self.add_seat_client(&object, pid);
            }
            session_core_seat::Event::Done | session_core_seat::Event::Ok => {
                self.seats[idx].done = true;
            }
            session_core_seat::Event::Destroyed => {
                self.seats[idx].destroyed = true;
                self.seats[idx].done = true;
            }
            session_core_seat::Event::Error { code: _, message } => {
                self.error = Some(message);
                self.seats[idx].done = true;
            }
        }
    }
}

impl hyprwire::Dispatch<session_core_seat_client::SessionCoreSeatClient> for State {
    fn event(
        &mut self,
        object: &session_core_seat_client::SessionCoreSeatClient,
        event: <session_core_seat_client::SessionCoreSeatClient as hyprwire::Object>::Event<'_>,
    ) {
        let Some(idx) = self
            .seat_clients
            .iter()
            .position(|client| client.object == *object)
        else {
            return;
        };

        match event {
            session_core_seat_client::Event::Session { session } => {
                self.seat_clients[idx].session = session;
            }
            session_core_seat_client::Event::State { state } => {
                self.seat_clients[idx].state = state;
            }
            session_core_seat_client::Event::Active { active } => {
                self.seat_clients[idx].active = active != 0;
            }
            session_core_seat_client::Event::Next { next } => {
                self.seat_clients[idx].next = next != 0;
            }
            session_core_seat_client::Event::Pid { pid } => self.seat_clients[idx].pid = pid,
            session_core_seat_client::Event::Uid { uid } => self.seat_clients[idx].uid = Some(uid),
            session_core_seat_client::Event::Gid { gid } => self.seat_clients[idx].gid = Some(gid),
            session_core_seat_client::Event::DeviceAdded { device_id } => {
                if !self.seat_clients[idx].devices.contains(&device_id) {
                    self.seat_clients[idx].devices.push(device_id);
                }
                let object = object.clone();
                self.add_seat_device(&object, device_id);
            }
            session_core_seat_client::Event::DeviceRemoved { device_id } => {
                self.seat_clients[idx]
                    .devices
                    .retain(|candidate| *candidate != device_id);
            }
            session_core_seat_client::Event::Done => self.seat_clients[idx].done = true,
            session_core_seat_client::Event::Destroyed => {
                self.seat_clients[idx].destroyed = true;
                self.seat_clients[idx].done = true;
            }
        }
    }
}

impl hyprwire::Dispatch<session_core_seat_device::SessionCoreSeatDevice> for State {
    fn event(
        &mut self,
        object: &session_core_seat_device::SessionCoreSeatDevice,
        event: <session_core_seat_device::SessionCoreSeatDevice as hyprwire::Object>::Event<'_>,
    ) {
        let Some(device) = self
            .seat_devices
            .iter_mut()
            .find(|device| device.object == *object)
        else {
            return;
        };

        match event {
            session_core_seat_device::Event::DeviceId { device_id } => device.id = device_id,
            session_core_seat_device::Event::Path { path } => device.path = path,
            session_core_seat_device::Event::Kind { kind } => device.kind = kind,
            session_core_seat_device::Event::Active { active } => device.active = active != 0,
            session_core_seat_device::Event::Done => device.done = true,
            session_core_seat_device::Event::Destroyed => {
                device.destroyed = true;
                device.done = true;
            }
        }
    }
}

#[derive(Subcommand)]
enum Command {
    #[command(
        subcommand,
        about = "inspect and control sessions",
        arg_required_else_help = true
    )]
    Session(SessionCommand),
    #[command(
        subcommand,
        about = "inspect and manage users",
        arg_required_else_help = true
    )]
    User(UserCommand),
    #[command(
        subcommand,
        about = "inspect seats and attached clients",
        arg_required_else_help = true
    )]
    Seat(SeatCommand),
    #[command(about = "reboot the system")]
    Reboot,
    #[command(about = "power off the system")]
    Poweroff,
    #[command(about = "suspend the system")]
    Suspend,
    #[command(about = "hibernate the system")]
    Hibernate,
    #[command(about = "generate shell completions")]
    Completions {
        #[arg(value_name = "SHELL")]
        shell: CompletionShell,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum CompletionShell {
    Bash,
    Elvish,
    Fish,
    Nushell,
    Zsh,
}

impl CompletionShell {
    fn generate(self, command: &mut clap::Command) {
        let mut stdout = std::io::stdout();

        match self {
            Self::Bash => generate(Shell::Bash, command, "sessionctl", &mut stdout),
            Self::Elvish => generate(Shell::Elvish, command, "sessionctl", &mut stdout),
            Self::Fish => generate(Shell::Fish, command, "sessionctl", &mut stdout),
            Self::Nushell => generate(Nushell, command, "sessionctl", &mut stdout),
            Self::Zsh => generate(Shell::Zsh, command, "sessionctl", &mut stdout),
        }
    }
}

#[derive(Subcommand)]
enum SessionCommand {
    #[command(about = "show live sessions")]
    List {
        #[arg(long, help = "emit machine-readable JSON output")]
        json: bool,
    },
    #[command(
        about = "ask sessions to exit",
        group(ArgGroup::new("target").required(true).args(["ids", "current"]))
    )]
    Close {
        #[arg(long, help = "target your current session")]
        current: bool,
        #[arg(value_name = "SESSION", help = "session id; omit with --current")]
        ids: Vec<u32>,
    },
    #[command(
        about = "force-stop sessions",
        group(ArgGroup::new("target").required(true).args(["ids", "current"]))
    )]
    Kill {
        #[arg(long, help = "target your current session")]
        current: bool,
        #[arg(value_name = "SESSION", help = "session id; omit with --current")]
        ids: Vec<u32>,
    },
    #[command(
        about = "lock sessions",
        group(ArgGroup::new("target").args(["ids", "all"]).multiple(false))
    )]
    Lock {
        #[arg(long, help = "target all sessions")]
        all: bool,
        #[arg(value_name = "SESSION", help = "session id; defaults to your session")]
        ids: Vec<u32>,
    },
    #[command(
        about = "unlock sessions",
        group(ArgGroup::new("target").args(["ids", "all"]).multiple(false))
    )]
    Unlock {
        #[arg(long, help = "target all sessions")]
        all: bool,
        #[arg(value_name = "SESSION", help = "session id; defaults to your session")]
        ids: Vec<u32>,
    },
}

#[derive(Subcommand)]
enum UserCommand {
    #[command(about = "list users from active sessions")]
    List {
        #[arg(long, help = "emit machine-readable JSON output")]
        json: bool,
    },
    #[command(
        about = "force-stop user sessions",
        group(ArgGroup::new("target").required(true).args(["uids", "current"]))
    )]
    Kill {
        #[arg(long, help = "target your current user")]
        current: bool,
        #[arg(value_name = "UID", help = "user id; omit with --current")]
        uids: Vec<u32>,
    },
    #[command(
        about = "ask user sessions to exit",
        group(ArgGroup::new("target").required(true).args(["uids", "current"]))
    )]
    Terminate {
        #[arg(long, help = "target your current user")]
        current: bool,
        #[arg(value_name = "UID", help = "user id; omit with --current")]
        uids: Vec<u32>,
    },
}

#[derive(Subcommand)]
enum SeatCommand {
    #[command(about = "list seats from active sessions")]
    List {
        #[arg(long, help = "emit machine-readable JSON output")]
        json: bool,
    },
    #[command(about = "show a seat tree with clients and devices")]
    Status {
        #[arg(
            value_name = "SEAT",
            help = "seat name; defaults when only one seat exists"
        )]
        seat: Option<String>,
        #[arg(long, help = "emit machine-readable JSON output")]
        json: bool,
    },
    #[command(
        about = "ask seat sessions to exit",
        group(ArgGroup::new("target").required(true).args(["seats", "current"]))
    )]
    Terminate {
        #[arg(long, help = "target your current seat")]
        current: bool,
        #[arg(value_name = "SEAT", help = "seat name; omit with --current")]
        seats: Vec<String>,
    },
}

#[derive(Parser)]
#[command(
    name = "sessionctl",
    bin_name = "sessionctl",
    author,
    version,
    about = "sessiond control tool",
    long_about = None,
    arg_required_else_help = true
)]
pub(crate) struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Clone, Copy)]
enum SessionAction {
    Close,
    Kill,
    Lock,
    Unlock,
}

impl SessionAction {
    const fn from_command(command: &SessionCommand) -> Option<Self> {
        match command {
            SessionCommand::List { .. } => None,
            SessionCommand::Close { .. } => Some(Self::Close),
            SessionCommand::Kill { .. } => Some(Self::Kill),
            SessionCommand::Lock { .. } => Some(Self::Lock),
            SessionCommand::Unlock { .. } => Some(Self::Unlock),
        }
    }

    fn ids(command: &SessionCommand) -> Option<&[u32]> {
        match command {
            SessionCommand::List { .. } => None,
            SessionCommand::Close { ids, .. }
            | SessionCommand::Kill { ids, .. }
            | SessionCommand::Lock { ids, .. }
            | SessionCommand::Unlock { ids, .. } => Some(ids),
        }
    }

    const fn targets_all(command: &SessionCommand) -> bool {
        match command {
            SessionCommand::List { .. }
            | SessionCommand::Close { .. }
            | SessionCommand::Kill { .. } => false,
            SessionCommand::Lock { all, .. } | SessionCommand::Unlock { all, .. } => *all,
        }
    }

    const fn targets_current(command: &SessionCommand) -> bool {
        match command {
            SessionCommand::List { .. } => false,
            SessionCommand::Close { current, .. } | SessionCommand::Kill { current, .. } => {
                *current
            }
            SessionCommand::Lock { all, ids } | SessionCommand::Unlock { all, ids } => {
                !*all && ids.is_empty()
            }
        }
    }

    fn send(self, object: &session_core_session::SessionCoreSession) {
        match self {
            Self::Close => object.send_terminate(),
            Self::Kill => object.send_kill(),
            Self::Lock => object.send_lock(),
            Self::Unlock => object.send_unlock(),
        }
    }

    const fn name(self) -> &'static str {
        match self {
            Self::Close => "close",
            Self::Kill => "kill",
            Self::Lock => "lock",
            Self::Unlock => "unlock",
        }
    }
}

#[derive(Clone, Copy)]
enum UserAction {
    Kill,
    Terminate,
}

impl UserAction {
    fn send(self, object: &session_core_user::SessionCoreUser) {
        match self {
            Self::Kill => object.send_kill(),
            Self::Terminate => object.send_terminate(),
        }
    }

    const fn name(self) -> &'static str {
        match self {
            Self::Kill => "kill",
            Self::Terminate => "terminate",
        }
    }
}

const fn active_state_name(state: session_core_v1::ActiveState) -> &'static str {
    match state {
        session_core_v1::ActiveState::Online => "online",
        session_core_v1::ActiveState::Active => "active",
        session_core_v1::ActiveState::Closing => "closing",
        session_core_v1::ActiveState::Opening => "opening",
    }
}

const fn session_class_name(class: session_core_v1::SessionClass) -> &'static str {
    match class {
        session_core_v1::SessionClass::User => "user",
        session_core_v1::SessionClass::Greeter => "greeter",
        session_core_v1::SessionClass::LockScreen => "lock-screen",
        session_core_v1::SessionClass::Background => "background",
        session_core_v1::SessionClass::BackgroundLight => "background-light",
        session_core_v1::SessionClass::Manager => "manager",
        session_core_v1::SessionClass::ManagerEarly => "manager-early",
    }
}

const fn user_state_name(state: session_core_v1::UserState) -> &'static str {
    match state {
        session_core_v1::UserState::Offline => "offline",
        session_core_v1::UserState::Lingering => "lingering",
        session_core_v1::UserState::Online => "online",
        session_core_v1::UserState::Active => "active",
        session_core_v1::UserState::Closing => "closing",
    }
}

const fn seat_mode_name(mode: session_core_v1::SeatMode) -> &'static str {
    match mode {
        session_core_v1::SeatMode::VtBound => "vt-bound",
        session_core_v1::SeatMode::NonVtBound => "non-vt-bound",
    }
}

const fn seat_client_state_name(state: session_core_v1::SeatClientState) -> &'static str {
    match state {
        session_core_v1::SeatClientState::New => "new",
        session_core_v1::SeatClientState::Enabled => "enabled",
        session_core_v1::SeatClientState::PendingDisabled => "pending-disabled",
        session_core_v1::SeatClientState::Disabled => "disabled",
    }
}

const fn seat_device_kind_name(kind: session_core_v1::SeatDeviceKind) -> &'static str {
    match kind {
        session_core_v1::SeatDeviceKind::Drm => "drm",
        session_core_v1::SeatDeviceKind::Evdev => "evdev",
        session_core_v1::SeatDeviceKind::Hidraw => "hidraw",
        session_core_v1::SeatDeviceKind::Wscons => "wscons",
        session_core_v1::SeatDeviceKind::Unknown => "unknown",
    }
}

fn process_name(pid: u32) -> Option<String> {
    let name = fs::read_to_string(format!("/proc/{pid}/comm")).ok()?;
    let name = name.trim();
    (!name.is_empty()).then(|| name.to_string())
}

fn print_json(value: &serde_json::Value) -> anyhow::Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    if let Command::Completions { shell } = cli.command {
        let mut command = Cli::command();
        shell.generate(&mut command);
        return Ok(());
    }

    let mut state = State {
        sessions: Vec::new(),
        users: Vec::new(),
        seats: Vec::new(),
        seat_clients: Vec::new(),
        seat_devices: Vec::new(),
        done: false,
        can_control_power: false,
        error: None,
    };

    let mut socket = client::Client::connect("/run/sessiond.sock")?;
    let event_queue = socket.new_event_queue();

    socket.add_implementation::<session_core_v1::SessionCoreV1>();

    event_queue.wait_for_handshake(&mut state)?;

    let manager =
        socket.bind::<session_core_manager::SessionCoreManager, _>(&event_queue, &mut state, 1)?;

    event_queue.roundtrip(&mut state)?;

    while state.sessions.iter().any(|session| !session.done) {
        event_queue.dispatch_events(&mut state, true)?;
    }

    if let Some(err) = state.error.take() {
        anyhow::bail!("{err}");
    }

    match cli.command {
        Command::Session(command) => match command {
            SessionCommand::List { json } => {
                let fmt_tty = |vt: u32| -> String {
                    if vt == 0 {
                        String::from("-")
                    } else {
                        format!("tty{vt}")
                    }
                };

                if json {
                    return print_json(&json!({
                        "sessions": state.sessions.iter().map(|session| {
                            let mut obj = serde_json::Map::new();
                            obj.insert("id".into(), json!(session.id));
                            obj.insert("uid".into(), json!(session.uid));
                            obj.insert("username".into(), json!(session.username));
                            obj.insert("class".into(), json!(session_class_name(session.class)));
                            if session.vt != 0 {
                                obj.insert("vt".into(), json!(session.vt));
                                obj.insert("tty".into(), json!(fmt_tty(session.vt)));
                            }
                            if let Some(seat) = &session.seat {
                                obj.insert("seat".into(), json!(seat));
                            }
                            obj.insert("leader".into(), json!(session.leader));
                            obj.insert("timestamp".into(), json!(session.timestamp));
                            obj.insert("active".into(), json!(active_state_name(session.active)));
                            obj.insert("can_modify".into(), json!(session.can_modify));
                            serde_json::Value::Object(obj)
                        }).collect::<Vec<_>>(),
                    }));
                }

                if state.sessions.is_empty() {
                    println!("No active sessions.");
                    return Ok(());
                }

                let fmt_ts = |ts: u32| -> String {
                    use std::time::{Duration, UNIX_EPOCH};
                    let Some(t) = UNIX_EPOCH.checked_add(Duration::from_secs(u64::from(ts))) else {
                        return String::from("?");
                    };
                    let Ok(elapsed) = t.elapsed() else {
                        return String::from("?");
                    };
                    let s = elapsed.as_secs();
                    let h = s / 3600;
                    let m = (s % 3600) / 60;
                    let s = s % 60;
                    if h > 0 {
                        format!("{h}h {m}min ago")
                    } else if m > 0 {
                        format!("{m}min {s}s ago")
                    } else {
                        format!("{s}s ago")
                    }
                };

                let id_w = state
                    .sessions
                    .iter()
                    .map(|s| s.id.to_string().len())
                    .max()
                    .unwrap_or(0)
                    .max(2);
                let uid_w = state
                    .sessions
                    .iter()
                    .map(|s| s.uid.to_string().len())
                    .max()
                    .unwrap_or(0)
                    .max(3);
                let user_w = state
                    .sessions
                    .iter()
                    .map(|s| s.username.len())
                    .max()
                    .unwrap_or(0)
                    .max(4);
                let class_w = state
                    .sessions
                    .iter()
                    .map(|s| session_class_name(s.class).len())
                    .max()
                    .unwrap_or(0)
                    .max(5);
                let vt_w = state
                    .sessions
                    .iter()
                    .map(|s| fmt_tty(s.vt).len())
                    .max()
                    .unwrap_or(0)
                    .max(3);
                let seat_w = state
                    .sessions
                    .iter()
                    .map(|s| s.seat.as_deref().unwrap_or("-").len())
                    .max()
                    .unwrap_or(0)
                    .max(4);
                let leader_w = state
                    .sessions
                    .iter()
                    .map(|s| s.leader.to_string().len())
                    .max()
                    .unwrap_or(0)
                    .max(6);
                let since_w = state
                    .sessions
                    .iter()
                    .map(|s| fmt_ts(s.timestamp).len())
                    .max()
                    .unwrap_or(0)
                    .max(5);
                let state_w = state
                    .sessions
                    .iter()
                    .map(|s| active_state_name(s.active).len())
                    .max()
                    .unwrap_or(0)
                    .max(5);

                println!(
                    "  {:<id_w$}  {:<uid_w$}  {:<user_w$}  {:<class_w$}  {:<vt_w$}  {:<seat_w$}  {:<leader_w$}  {:<since_w$}  {:<state_w$}",
                    "ID", "UID", "USER", "CLASS", "TTY", "SEAT", "LEADER", "SINCE", "STATE",
                );
                println!(
                    "  {}",
                    "─".repeat(
                        id_w + 2
                            + uid_w
                            + 2
                            + user_w
                            + 2
                            + class_w
                            + 2
                            + vt_w
                            + 2
                            + seat_w
                            + 2
                            + leader_w
                            + 2
                            + since_w
                            + 2
                            + state_w
                    )
                );
                for session in state.sessions {
                    println!(
                        "  {:<id_w$}  {:<uid_w$}  {:<user_w$}  {:<class_w$}  {:<vt_w$}  {:<seat_w$}  {:<leader_w$}  {:<since_w$}  {:<state_w$}",
                        session.id,
                        session.uid,
                        session.username,
                        session_class_name(session.class),
                        fmt_tty(session.vt),
                        session.seat.as_deref().unwrap_or("-"),
                        session.leader,
                        fmt_ts(session.timestamp),
                        active_state_name(session.active),
                    );
                }
            }
            command @ (SessionCommand::Close { .. }
            | SessionCommand::Kill { .. }
            | SessionCommand::Lock { .. }
            | SessionCommand::Unlock { .. }) => {
                let Some(action) = SessionAction::from_command(&command) else {
                    unreachable!();
                };
                let Some(ids) = SessionAction::ids(&command) else {
                    unreachable!();
                };

                if SessionAction::targets_current(&command) {
                    let uid = process::getuid();

                    let session = state
                        .sessions
                        .iter()
                        .find(|session| session.uid == uid.as_raw())
                        .context(format!(
                            "{} Session for uid {} not found, check if pam module is installed",
                            "error:".red(),
                            uid.as_raw()
                        ))?;
                    if !session.can_modify {
                        eprintln!(
                            "{} cannot {} session {}: permission denied",
                            "error:".red(),
                            action.name(),
                            session.id
                        );
                        return Ok(());
                    }
                    action.send(&session.object);

                    return Ok(());
                }

                let mut unmatched: Vec<&u32> = Vec::new();
                let mut matched = Vec::new();
                let mut denied = Vec::new();

                if SessionAction::targets_all(&command) {
                    for session in &state.sessions {
                        if session.can_modify {
                            matched.push(session.object.clone());
                        } else {
                            denied.push(session.id);
                        }
                    }
                } else {
                    for id in ids {
                        if let Some(session) =
                            state.sessions.iter().find(|session| session.id == *id)
                        {
                            if session.can_modify {
                                matched.push(session.object.clone());
                            } else {
                                denied.push(session.id);
                            }
                        } else {
                            unmatched.push(id);
                        }
                    }
                }

                if !unmatched.is_empty() {
                    let ids = unmatched
                        .iter()
                        .map(std::string::ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(", ");
                    eprintln!("{} unknown session ids: {ids}", "error:".red());
                    return Ok(());
                }

                if !denied.is_empty() {
                    let ids = denied
                        .iter()
                        .map(std::string::ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(", ");
                    eprintln!(
                        "{} cannot {} session ids without permission: {ids}",
                        "error:".red(),
                        action.name()
                    );
                    return Ok(());
                }

                if matched.is_empty() {
                    return Ok(());
                }

                for obj in matched {
                    action.send(&obj);
                }

                while !state.done {
                    event_queue.dispatch_events(&mut state, true)?;
                }

                if let Some(err) = state.error {
                    eprintln!("{} {err}", "error:".red());
                }
            }
        },
        Command::User(command) => match command {
            UserCommand::List { json } => {
                let mut uids = state
                    .sessions
                    .iter()
                    .map(|session| session.uid)
                    .collect::<Vec<_>>();
                uids.sort_unstable();
                uids.dedup();

                for uid in uids {
                    state.add_user(&manager, uid);
                }

                while state.users.iter().any(|user| !user.done) {
                    event_queue.dispatch_events(&mut state, true)?;
                }

                if let Some(err) = state.error.take() {
                    anyhow::bail!("{err}");
                }

                let users = state
                    .users
                    .iter()
                    .filter(|user| !user.not_found)
                    .collect::<Vec<_>>();

                if json {
                    return print_json(&json!({
                        "users": users.iter().map(|user| {
                            json!({
                                "uid": user.uid,
                                "username": user.username,
                                "lingering": user.lingering,
                                "state": user_state_name(user.state),
                                "can_modify": user.can_modify,
                            })
                        }).collect::<Vec<_>>(),
                    }));
                }

                if users.is_empty() {
                    println!("No users found.");
                    return Ok(());
                }

                let uid_w = users
                    .iter()
                    .map(|user| user.uid.to_string().len())
                    .max()
                    .unwrap_or(0)
                    .max(3);
                let user_w = users
                    .iter()
                    .map(|user| user.username.len())
                    .max()
                    .unwrap_or(0)
                    .max(4);
                let state_w = users
                    .iter()
                    .map(|user| user_state_name(user.state).len())
                    .max()
                    .unwrap_or(0)
                    .max(5);

                println!(
                    "  {:<uid_w$}  {:<user_w$}  {:<state_w$}  LINGER",
                    "UID", "USER", "STATE"
                );
                println!("  {}", "─".repeat(uid_w + 2 + user_w + 2 + state_w + 2 + 6));
                for user in users {
                    println!(
                        "  {:<uid_w$}  {:<user_w$}  {:<state_w$}  {}",
                        user.uid,
                        user.username,
                        user_state_name(user.state),
                        if user.lingering { "yes" } else { "no" },
                    );
                }
            }
            UserCommand::Kill { current, uids } => {
                let action = UserAction::Kill;
                let uids = if current {
                    vec![process::getuid().as_raw()]
                } else {
                    uids
                };

                for uid in &uids {
                    state.add_user(&manager, *uid);
                }

                while state.users.iter().any(|user| !user.done) {
                    event_queue.dispatch_events(&mut state, true)?;
                }

                if let Some(err) = state.error.take() {
                    anyhow::bail!("{err}");
                }

                let unmatched = state
                    .users
                    .iter()
                    .filter(|user| user.not_found)
                    .map(|user| user.uid)
                    .collect::<Vec<_>>();

                if !unmatched.is_empty() {
                    let uids = unmatched
                        .iter()
                        .map(std::string::ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(", ");
                    eprintln!("{} unknown user ids: {uids}", "error:".red());
                    return Ok(());
                }

                let denied = state
                    .users
                    .iter()
                    .filter(|user| !user.not_found && !user.can_modify)
                    .map(|user| user.uid)
                    .collect::<Vec<_>>();

                if !denied.is_empty() {
                    let uids = denied
                        .iter()
                        .map(std::string::ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(", ");
                    eprintln!(
                        "{} cannot {} user ids without permission: {uids}",
                        "error:".red(),
                        action.name()
                    );
                    return Ok(());
                }

                for user in &mut state.users {
                    user.done = false;
                    action.send(&user.object);
                }

                while state.users.iter().any(|user| !user.done) {
                    event_queue.dispatch_events(&mut state, true)?;
                }

                if let Some(err) = state.error {
                    eprintln!("{} {err}", "error:".red());
                }
            }
            UserCommand::Terminate { current, uids } => {
                let action = UserAction::Terminate;
                let uids = if current {
                    vec![process::getuid().as_raw()]
                } else {
                    uids
                };

                for uid in &uids {
                    state.add_user(&manager, *uid);
                }

                while state.users.iter().any(|user| !user.done) {
                    event_queue.dispatch_events(&mut state, true)?;
                }

                if let Some(err) = state.error.take() {
                    anyhow::bail!("{err}");
                }

                let unmatched = state
                    .users
                    .iter()
                    .filter(|user| user.not_found)
                    .map(|user| user.uid)
                    .collect::<Vec<_>>();

                if !unmatched.is_empty() {
                    let uids = unmatched
                        .iter()
                        .map(std::string::ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(", ");
                    eprintln!("{} unknown user ids: {uids}", "error:".red());
                    return Ok(());
                }

                let denied = state
                    .users
                    .iter()
                    .filter(|user| !user.not_found && !user.can_modify)
                    .map(|user| user.uid)
                    .collect::<Vec<_>>();

                if !denied.is_empty() {
                    let uids = denied
                        .iter()
                        .map(std::string::ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(", ");
                    eprintln!(
                        "{} cannot {} user ids without permission: {uids}",
                        "error:".red(),
                        action.name()
                    );
                    return Ok(());
                }

                for user in &mut state.users {
                    user.done = false;
                    action.send(&user.object);
                }

                while state.users.iter().any(|user| !user.done) {
                    event_queue.dispatch_events(&mut state, true)?;
                }

                if let Some(err) = state.error {
                    eprintln!("{} {err}", "error:".red());
                }
            }
        },
        Command::Seat(command) => {
            let mut seat_names = state
                .sessions
                .iter()
                .filter_map(|session| session.seat.clone())
                .collect::<Vec<_>>();
            seat_names.sort();
            seat_names.dedup();

            for seat in &seat_names {
                state.add_seat(&manager, seat.clone());
            }

            while state.seats.iter().any(|seat| !seat.done)
                || state.seat_clients.iter().any(|client| !client.done)
                || state.seat_devices.iter().any(|device| !device.done)
            {
                event_queue.dispatch_events(&mut state, true)?;
            }

            if let Some(err) = state.error.take() {
                anyhow::bail!("{err}");
            }

            match command {
                SeatCommand::List { json } => {
                    let seats = state
                        .seats
                        .iter()
                        .filter(|seat| !seat.destroyed)
                        .collect::<Vec<_>>();

                    if json {
                        return print_json(&json!({
                            "seats": seats.iter().map(|seat| {
                                json!({
                                    "name": seat.name,
                                    "mode": seat_mode_name(seat.mode),
                                    "current_vt": seat.current_vt,
                                    "clients": seat.clients.len(),
                                    "can_modify": seat.can_modify,
                                })
                            }).collect::<Vec<_>>(),
                        }));
                    }

                    if seats.is_empty() {
                        println!("No seats found from active sessions.");
                        return Ok(());
                    }

                    let name_w = seats
                        .iter()
                        .map(|seat| seat.name.len())
                        .max()
                        .unwrap_or(0)
                        .max(4);
                    let mode_w = seats
                        .iter()
                        .map(|seat| seat_mode_name(seat.mode).len())
                        .max()
                        .unwrap_or(0)
                        .max(4);
                    println!(
                        "  {:<name_w$}  {:<mode_w$}  {:<3}  CLIENTS",
                        "SEAT", "MODE", "VT"
                    );
                    println!("  {}", "─".repeat(name_w + 2 + mode_w + 2 + 3 + 2 + 7));
                    for seat in seats {
                        println!(
                            "  {:<name_w$}  {:<mode_w$}  {:<3}  {}",
                            seat.name,
                            seat_mode_name(seat.mode),
                            if seat.current_vt == 0 {
                                "-".to_string()
                            } else {
                                seat.current_vt.to_string()
                            },
                            seat.clients.len(),
                        );
                    }
                }
                SeatCommand::Status { seat, json } => {
                    let seat_name = match seat {
                        Some(seat) => seat,
                        None if seat_names.len() == 1 => seat_names[0].clone(),
                        None if seat_names.is_empty() => {
                            eprintln!("{} no seats found from active sessions", "error:".red());
                            return Ok(());
                        }
                        None => {
                            eprintln!(
                                "{} multiple seats found; specify one of: {}",
                                "error:".red(),
                                seat_names.join(", ")
                            );
                            return Ok(());
                        }
                    };

                    let Some(seat) = state
                        .seats
                        .iter()
                        .find(|candidate| candidate.name == seat_name && !candidate.destroyed)
                    else {
                        eprintln!("{} unknown seat: {seat_name}", "error:".red());
                        return Ok(());
                    };

                    let clients = state
                        .seat_clients
                        .iter()
                        .filter(|client| client.seat == seat.name && !client.destroyed)
                        .collect::<Vec<_>>();

                    if json {
                        return print_json(&json!({
                            "seat": {
                                "name": seat.name,
                                "mode": seat_mode_name(seat.mode),
                                "current_vt": seat.current_vt,
                                "can_modify": seat.can_modify,
                                "clients": clients.iter().map(|client| {
                                    let process_name = process_name(client.pid);
                                    json!({
                                        "pid": client.pid,
                                        "process": process_name,
                                        "session": client.session,
                                        "state": seat_client_state_name(client.state),
                                        "active": client.active,
                                        "next": client.next,
                                        "uid": client.uid,
                                        "gid": client.gid,
                                        "devices": state.seat_devices.iter()
                                            .filter(|device| device.client_pid == client.pid && !device.destroyed)
                                            .map(|device| json!({
                                                "id": device.id,
                                                "path": device.path,
                                                "kind": seat_device_kind_name(device.kind),
                                                "active": device.active,
                                            }))
                                            .collect::<Vec<_>>(),
                                    })
                                }).collect::<Vec<_>>(),
                            }
                        }));
                    }

                    let vt = if seat.current_vt == 0 {
                        "-".to_string()
                    } else {
                        seat.current_vt.to_string()
                    };
                    println!(
                        "{} {}  mode={}  vt={}",
                        "Seat".bold(),
                        seat.name.bold(),
                        seat_mode_name(seat.mode),
                        vt
                    );

                    if clients.is_empty() {
                        println!("└─ no clients");
                        return Ok(());
                    }

                    for (client_idx, client) in clients.iter().enumerate() {
                        let last_client = client_idx + 1 == clients.len();
                        let client_prefix = if last_client { "└─" } else { "├─" };
                        let child_prefix = if last_client { "  " } else { "│ " };
                        let mut flags = Vec::new();
                        if client.active {
                            flags.push("active");
                        }
                        if client.next {
                            flags.push("next");
                        }
                        if client.state != session_core_v1::SeatClientState::Enabled {
                            flags.push(seat_client_state_name(client.state));
                        }
                        let flags = if flags.is_empty() {
                            String::new()
                        } else {
                            format!(" [{}]", flags.join(", "))
                        };
                        println!(
                            "{client_prefix} client pid={}{} session={}{}",
                            client.pid,
                            process_name(client.pid)
                                .map(|name| format!(" ({name})"))
                                .unwrap_or_default(),
                            client.session,
                            flags
                        );

                        let devices = state
                            .seat_devices
                            .iter()
                            .filter(|device| device.client_pid == client.pid && !device.destroyed)
                            .collect::<Vec<_>>();
                        if devices.is_empty() {
                            println!("{child_prefix}└─ no devices");
                            continue;
                        }

                        for (device_idx, device) in devices.iter().enumerate() {
                            let device_prefix = if device_idx + 1 == devices.len() {
                                "└─"
                            } else {
                                "├─"
                            };
                            println!(
                                "{child_prefix}{device_prefix} device #{} {} {} {}",
                                device.id,
                                seat_device_kind_name(device.kind),
                                if device.active { "active" } else { "inactive" },
                                if device.path.is_empty() {
                                    "<unknown>"
                                } else {
                                    &device.path
                                }
                            );
                        }
                    }
                }
                SeatCommand::Terminate { current, seats } => {
                    let seats = if current {
                        let uid = process::getuid();
                        let session = state
                            .sessions
                            .iter()
                            .find(|session| session.uid == uid.as_raw())
                            .context(format!(
                                "{} Session for uid {} not found, check if pam module is installed",
                                "error:".red(),
                                uid.as_raw()
                            ))?;
                        let seat = session.seat.clone().context(format!(
                            "{} current session is not assigned to a seat",
                            "error:".red()
                        ))?;
                        vec![seat]
                    } else {
                        seats
                    };

                    let mut unmatched = Vec::new();
                    let mut matched = Vec::new();
                    let mut denied = Vec::new();

                    for seat_name in &seats {
                        if let Some(seat) = state
                            .seats
                            .iter()
                            .find(|seat| seat.name == *seat_name && !seat.destroyed)
                        {
                            if seat.can_modify {
                                matched.push(seat.object.clone());
                            } else {
                                denied.push(seat.name.clone());
                            }
                        } else {
                            unmatched.push(seat_name);
                        }
                    }

                    if !unmatched.is_empty() {
                        let seats = unmatched
                            .iter()
                            .map(std::string::ToString::to_string)
                            .collect::<Vec<_>>()
                            .join(", ");
                        eprintln!("{} unknown seats: {seats}", "error:".red());
                        return Ok(());
                    }

                    if !denied.is_empty() {
                        eprintln!(
                            "{} cannot terminate seats without permission: {}",
                            "error:".red(),
                            denied.join(", ")
                        );
                        return Ok(());
                    }

                    for seat in &mut state.seats {
                        if matched.iter().any(|object| object == &seat.object) {
                            seat.done = false;
                        }
                    }
                    for object in matched {
                        object.send_terminate();
                    }

                    while state.seats.iter().any(|seat| !seat.done) {
                        event_queue.dispatch_events(&mut state, true)?;
                    }

                    if let Some(err) = state.error {
                        eprintln!("{} {err}", "error:".red());
                    }
                }
            }
        }
        Command::Completions { shell: _ } => unreachable!(),
        cmd => {
            let command_name = match cmd {
                Command::Reboot => "reboot",
                Command::Poweroff => "power off",
                Command::Hibernate => "hibernate",
                Command::Suspend => "suspend",
                _ => unreachable!(),
            };
            if !state.can_control_power {
                eprintln!(
                    "{} cannot {command_name} while other users have active sessions",
                    "error:".red()
                );
                return Ok(());
            }

            state.done = false;
            state.error = None;

            match cmd {
                Command::Reboot => manager.send_reboot(),
                Command::Poweroff => manager.send_poweroff(),
                Command::Hibernate => manager.send_hibernate(),
                Command::Suspend => manager.send_suspend(),
                _ => unreachable!(),
            }

            while !state.done {
                event_queue.dispatch_events(&mut state, true)?;
            }

            if let Some(err) = state.error {
                eprintln!("{} {err}", "error:".red());
            }
        }
    }

    Ok(())
}
