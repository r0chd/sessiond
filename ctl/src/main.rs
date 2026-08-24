mod session_core_v1 {
    hyprwire::include_protocol!("session_core_v1");
    pub use client::*;
    pub use spec::*;
}
mod session_power_v1 {
    hyprwire::include_protocol!("session_power_v1");
    pub use client::*;
}
mod seat_core_v1 {
    hyprwire::include_protocol!("seat_core_v1");
    pub use client::*;
    pub use spec::*;
}

use clap::{CommandFactory, FromArgMatches, Parser, Subcommand, ValueEnum};
use comfy_table::presets;
use hyprwire::client;
use seat_core_v1::{seat_core_client, seat_core_device, seat_core_manager, seat_core_seat};
use serde_json::json;
use session_core_v1::{session_core_manager, session_core_session, session_core_user};
use session_power_v1::session_power_manager;
use std::{fs, io, time};

struct Session {
    object: Option<session_core_session::SessionCoreSession>,
    id: u32,
    uid: u32,
    username: String,
    class: session_core_v1::SessionClass,
    vt: u32,
    leader: u32,
    timestamp: u32,
    active: session_core_v1::ActiveState,
    can_modify: bool,
    done: bool,
}

struct User {
    object: Option<session_core_user::SessionCoreUser>,
    uid: u32,
    username: String,
    lingering: bool,
    state: session_core_v1::UserState,
    can_modify: bool,
    done: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SnapshotState {
    Pending,
    Complete,
    Destroyed,
}

struct Seat {
    object: seat_core_seat::SeatCoreSeat,
    name: String,
    mode: seat_core_v1::SeatMode,
    current_vt: u32,
    clients: Vec<u32>,
    snapshot: SnapshotState,
}

struct SeatClient {
    object: seat_core_client::SeatCoreClient,
    seat: String,
    session: u32,
    state: seat_core_v1::ClientState,
    active: bool,
    next: bool,
    pid: u32,
    uid: u32,
    gid: u32,
    devices: Vec<u32>,
    snapshot: SnapshotState,
}

struct SeatDevice {
    object: seat_core_device::SeatCoreDevice,
    seat: String,
    client_pid: u32,
    id: u32,
    path: String,
    kind: seat_core_v1::DeviceKind,
    active: bool,
    snapshot: SnapshotState,
}

#[derive(Default)]
struct SeatState {
    seats: Vec<Seat>,
    clients: Vec<SeatClient>,
    devices: Vec<SeatDevice>,
    manager_done: bool,
    error: Option<String>,
}

#[derive(Default)]
struct State {
    sessions: Vec<Session>,
    users: Vec<User>,
    done: bool,
    can_control_power: Option<bool>,
    error: Option<String>,
}

impl State {
    fn session(&mut self, manager: &session_core_manager::SessionCoreManager, id: u32) {
        if self.sessions.iter().any(|s| s.id == id) {
            return;
        }
        if let Some(object) = manager.send_get_session::<Self>(id) {
            let s = Session {
                object: Some(object),
                id,
                uid: 0,
                username: String::new(),
                class: session_core_v1::SessionClass::User,
                vt: 0,
                leader: 0,
                timestamp: 0,
                active: session_core_v1::ActiveState::Online,
                can_modify: false,
                done: false,
            };
            self.sessions.push(s);
        }
    }
    fn user(&mut self, manager: &session_core_manager::SessionCoreManager, uid: u32) {
        if self.users.iter().any(|u| u.uid == uid) {
            return;
        }
        if let Some(object) = manager.send_get_user::<Self>(uid) {
            self.users.push(User {
                object: Some(object),
                uid,
                username: String::new(),
                lingering: false,
                state: session_core_v1::UserState::Offline,
                can_modify: false,
                done: false,
            });
        }
    }
}

impl SeatState {
    fn add_seat(&mut self, manager: &seat_core_manager::SeatCoreManager, name: String) {
        if self.seats.iter().any(|seat| seat.name == name) {
            return;
        }
        let Some(object) = manager.send_get_seat::<Self>(&name) else {
            self.error = Some(format!("failed to create seat object for {name}"));
            return;
        };
        self.seats.push(Seat {
            object,
            name,
            mode: seat_core_v1::SeatMode::NonVtBound,
            current_vt: 0,
            clients: Vec::new(),
            snapshot: SnapshotState::Pending,
        });
    }

    fn add_client(&mut self, seat_object: &seat_core_seat::SeatCoreSeat, pid: u32) {
        let Some(seat) = self.seats.iter().find(|seat| seat.object == *seat_object) else {
            return;
        };
        if self
            .clients
            .iter()
            .any(|client| client.seat == seat.name && client.pid == pid)
        {
            return;
        }
        let seat_name = seat.name.clone();
        let Some(object) = seat.object.send_get_client::<Self>(pid) else {
            return;
        };
        self.clients.push(SeatClient {
            object,
            seat: seat_name,
            session: 0,
            state: seat_core_v1::ClientState::New,
            active: false,
            next: false,
            pid,
            uid: 0,
            gid: 0,
            devices: Vec::new(),
            snapshot: SnapshotState::Pending,
        });
    }

    fn add_device(&mut self, client_object: &seat_core_client::SeatCoreClient, id: u32) {
        let Some(client) = self
            .clients
            .iter()
            .find(|client| client.object == *client_object)
        else {
            return;
        };
        if self.devices.iter().any(|device| {
            device.seat == client.seat && (device.client_pid, device.id) == (client.pid, id)
        }) {
            return;
        }
        let seat = client.seat.clone();
        let client_pid = client.pid;
        let Some(object) = client.object.send_get_device::<Self>(id) else {
            return;
        };
        self.devices.push(SeatDevice {
            object,
            seat,
            client_pid,
            id,
            path: String::new(),
            kind: seat_core_v1::DeviceKind::Unknown,
            active: false,
            snapshot: SnapshotState::Pending,
        });
    }

    fn initial_dump_done(&self) -> bool {
        self.manager_done
            && self
                .seats
                .iter()
                .all(|seat| seat.snapshot != SnapshotState::Pending)
            && self
                .clients
                .iter()
                .all(|client| client.snapshot != SnapshotState::Pending)
            && self
                .devices
                .iter()
                .all(|device| device.snapshot != SnapshotState::Pending)
    }
}

impl hyprwire::Dispatch<seat_core_manager::SeatCoreManager> for SeatState {
    fn event(
        &mut self,
        object: &seat_core_manager::SeatCoreManager,
        event: <seat_core_manager::SeatCoreManager as hyprwire::Object>::Event<'_>,
    ) {
        match event {
            seat_core_manager::Event::SeatAdded { seat } => self.add_seat(object, seat),
            seat_core_manager::Event::Done => self.manager_done = true,
            seat_core_manager::Event::Error { message, .. } => {
                self.error = Some(message);
                self.manager_done = true;
            }
        }
    }
}

impl hyprwire::Dispatch<seat_core_seat::SeatCoreSeat> for SeatState {
    fn event(
        &mut self,
        object: &seat_core_seat::SeatCoreSeat,
        event: <seat_core_seat::SeatCoreSeat as hyprwire::Object>::Event<'_>,
    ) {
        let Some(index) = self.seats.iter().position(|seat| seat.object == *object) else {
            return;
        };
        match event {
            seat_core_seat::Event::Seat { seat } => self.seats[index].name = seat,
            seat_core_seat::Event::Mode { mode } => self.seats[index].mode = mode,
            seat_core_seat::Event::CurrentVt { vt } => self.seats[index].current_vt = vt,
            seat_core_seat::Event::ClientAdded { pid } => {
                if !self.seats[index].clients.contains(&pid) {
                    self.seats[index].clients.push(pid);
                }
                let object = object.clone();
                self.add_client(&object, pid);
            }
            seat_core_seat::Event::ClientRemoved { pid } => {
                self.seats[index]
                    .clients
                    .retain(|candidate| *candidate != pid);
                let seat = self.seats[index].name.clone();
                for client in self
                    .clients
                    .iter_mut()
                    .filter(|client| client.seat == seat && client.pid == pid)
                {
                    client.snapshot = SnapshotState::Destroyed;
                }
                for device in self
                    .devices
                    .iter_mut()
                    .filter(|device| device.seat == seat && device.client_pid == pid)
                {
                    device.snapshot = SnapshotState::Destroyed;
                }
            }
            seat_core_seat::Event::Done => {
                self.seats[index].snapshot = SnapshotState::Complete;
            }
            seat_core_seat::Event::Destroyed => {
                let seat = self.seats[index].name.clone();
                self.seats[index].snapshot = SnapshotState::Destroyed;
                for client in self.clients.iter_mut().filter(|client| client.seat == seat) {
                    client.snapshot = SnapshotState::Destroyed;
                }
                for device in self.devices.iter_mut().filter(|device| device.seat == seat) {
                    device.snapshot = SnapshotState::Destroyed;
                }
            }
            seat_core_seat::Event::Error { message, .. } => {
                self.error = Some(message);
                self.seats[index].snapshot = SnapshotState::Complete;
            }
        }
    }
}

impl hyprwire::Dispatch<seat_core_client::SeatCoreClient> for SeatState {
    fn event(
        &mut self,
        object: &seat_core_client::SeatCoreClient,
        event: <seat_core_client::SeatCoreClient as hyprwire::Object>::Event<'_>,
    ) {
        let Some(index) = self
            .clients
            .iter()
            .position(|client| client.object == *object)
        else {
            return;
        };
        match event {
            seat_core_client::Event::Session { session } => self.clients[index].session = session,
            seat_core_client::Event::State { state } => self.clients[index].state = state,
            seat_core_client::Event::Active { active } => {
                self.clients[index].active = active != 0;
            }
            seat_core_client::Event::Next { next } => self.clients[index].next = next != 0,
            seat_core_client::Event::Pid { pid } => self.clients[index].pid = pid,
            seat_core_client::Event::Uid { uid } => self.clients[index].uid = uid,
            seat_core_client::Event::Gid { gid } => self.clients[index].gid = gid,
            seat_core_client::Event::DeviceAdded { device_id } => {
                if !self.clients[index].devices.contains(&device_id) {
                    self.clients[index].devices.push(device_id);
                }
                let object = object.clone();
                self.add_device(&object, device_id);
            }
            seat_core_client::Event::DeviceRemoved { device_id } => {
                self.clients[index]
                    .devices
                    .retain(|candidate| *candidate != device_id);
                let seat = self.clients[index].seat.clone();
                let pid = self.clients[index].pid;
                for device in self.devices.iter_mut().filter(|device| {
                    device.seat == seat && device.client_pid == pid && device.id == device_id
                }) {
                    device.snapshot = SnapshotState::Destroyed;
                }
            }
            seat_core_client::Event::Done => {
                self.clients[index].snapshot = SnapshotState::Complete;
            }
            seat_core_client::Event::Destroyed => {
                let seat = self.clients[index].seat.clone();
                let pid = self.clients[index].pid;
                self.clients[index].snapshot = SnapshotState::Destroyed;
                for device in self
                    .devices
                    .iter_mut()
                    .filter(|device| device.seat == seat && device.client_pid == pid)
                {
                    device.snapshot = SnapshotState::Destroyed;
                }
            }
        }
    }
}

impl hyprwire::Dispatch<seat_core_device::SeatCoreDevice> for SeatState {
    fn event(
        &mut self,
        object: &seat_core_device::SeatCoreDevice,
        event: <seat_core_device::SeatCoreDevice as hyprwire::Object>::Event<'_>,
    ) {
        let Some(device) = self
            .devices
            .iter_mut()
            .find(|device| device.object == *object)
        else {
            return;
        };
        match event {
            seat_core_device::Event::DeviceId { device_id } => device.id = device_id,
            seat_core_device::Event::Path { path } => device.path = path,
            seat_core_device::Event::Kind { kind } => device.kind = kind,
            seat_core_device::Event::Active { active } => device.active = active != 0,
            seat_core_device::Event::Done => device.snapshot = SnapshotState::Complete,
            seat_core_device::Event::Destroyed => {
                device.snapshot = SnapshotState::Destroyed;
            }
        }
    }
}

impl hyprwire::Dispatch<session_core_manager::SessionCoreManager> for State {
    fn event(
        &mut self,
        object: &session_core_manager::SessionCoreManager,
        event: <session_core_manager::SessionCoreManager as hyprwire::Object>::Event<'_>,
    ) {
        match event {
            session_core_manager::Event::SessionAdded { session_id } => {
                self.session(object, session_id);
                self.done = false;
            }
            session_core_manager::Event::UserAdded { uid } => {
                self.user(object, uid);
                self.done = false;
            }
            session_core_manager::Event::Done => self.done = true,
            session_core_manager::Event::Error { message, .. } => {
                self.error = Some(message);
                self.done = true;
            }
        }
    }
}
impl hyprwire::Dispatch<session_power_manager::SessionPowerManager> for State {
    fn event(
        &mut self,
        _object: &session_power_manager::SessionPowerManager,
        event: <session_power_manager::SessionPowerManager as hyprwire::Object>::Event<'_>,
    ) {
        match event {
            session_power_manager::Event::CanControl { can_control } => {
                self.can_control_power = Some(can_control != 0);
            }
            session_power_manager::Event::Error { message, .. } => {
                self.error = Some(message);
            }
            session_power_manager::Event::Done => {}
        }
    }
}
impl hyprwire::Dispatch<session_core_session::SessionCoreSession> for State {
    fn event(
        &mut self,
        object: &session_core_session::SessionCoreSession,
        event: <session_core_session::SessionCoreSession as hyprwire::Object>::Event<'_>,
    ) {
        let Some(s) = self
            .sessions
            .iter_mut()
            .find(|s| s.object.as_ref() == Some(object))
        else {
            return;
        };
        match event {
            session_core_session::Event::SessionId { session_id } => s.id = session_id,
            session_core_session::Event::Uid { uid } => s.uid = uid,
            session_core_session::Event::Username { username } => s.username = username,
            session_core_session::Event::Class { class } => s.class = class,
            session_core_session::Event::Vt { vt } => s.vt = vt,
            session_core_session::Event::Leader { leader } => s.leader = leader,
            session_core_session::Event::Timestamp { timestamp } => s.timestamp = timestamp,
            session_core_session::Event::Active { state } => s.active = state,
            session_core_session::Event::CanModify { can_modify } => s.can_modify = can_modify != 0,
            session_core_session::Event::Done | session_core_session::Event::Destroyed => {
                s.done = true;
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
        let Some(u) = self
            .users
            .iter_mut()
            .find(|u| u.object.as_ref() == Some(object))
        else {
            return;
        };
        match event {
            session_core_user::Event::Uid { uid } => u.uid = uid,
            session_core_user::Event::Username { username } => u.username = username,
            session_core_user::Event::Lingering { lingering } => u.lingering = lingering != 0,
            session_core_user::Event::State { state } => u.state = state,
            session_core_user::Event::CanModify { can_modify } => u.can_modify = can_modify != 0,
            session_core_user::Event::Done | session_core_user::Event::NotFound => u.done = true,
            _ => {}
        }
    }
}

#[derive(Subcommand)]
enum SessionCommand {
    List {
        #[arg(long)]
        json: bool,
    },
    Close {
        ids: Vec<u32>,
    },
    Kill {
        ids: Vec<u32>,
    },
}
#[derive(Subcommand)]
enum UserCommand {
    List {
        #[arg(long)]
        json: bool,
    },
    Terminate {
        uids: Vec<u32>,
    },
    Kill {
        uids: Vec<u32>,
    },
}
#[derive(Subcommand)]
enum SeatCommand {
    #[command(about = "list seats managed by sessiond-seat")]
    List {
        #[arg(long, help = "emit machine-readable JSON output")]
        json: bool,
    },
    #[command(about = "show a seat's clients and devices")]
    Status {
        #[arg(
            value_name = "SEAT",
            help = "seat name; defaults when exactly one seat exists"
        )]
        seat: Option<String>,
        #[arg(long, help = "emit machine-readable JSON output")]
        json: bool,
    },
}
#[derive(Subcommand)]
enum Command {
    Session {
        #[command(subcommand)]
        command: SessionCommand,
    },
    User {
        #[command(subcommand)]
        command: UserCommand,
    },
    #[command(about = "inspect seats, clients, and devices")]
    Seat {
        #[command(subcommand)]
        command: SeatCommand,
    },
    Reboot,
    Poweroff,
    Suspend,
    Hibernate,
    Completions {
        shell: CompletionShell,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum CompletionShell {
    Bash,
    Elvish,
    Fish,
    Nushell,
    Powershell,
    Zsh,
}

impl CompletionShell {
    fn generate(self, command: &mut clap::Command) {
        let mut stdout = io::stdout();
        match self {
            Self::Bash => clap_complete::generate(
                clap_complete::Shell::Bash,
                command,
                "sessionctl",
                &mut stdout,
            ),
            Self::Elvish => clap_complete::generate(
                clap_complete::Shell::Elvish,
                command,
                "sessionctl",
                &mut stdout,
            ),
            Self::Fish => clap_complete::generate(
                clap_complete::Shell::Fish,
                command,
                "sessionctl",
                &mut stdout,
            ),
            Self::Nushell => clap_complete::generate(
                clap_complete_nushell::Nushell,
                command,
                "sessionctl",
                &mut stdout,
            ),
            Self::Powershell => clap_complete::generate(
                clap_complete::Shell::PowerShell,
                command,
                "sessionctl",
                &mut stdout,
            ),
            Self::Zsh => clap_complete::generate(
                clap_complete::Shell::Zsh,
                command,
                "sessionctl",
                &mut stdout,
            ),
        }
    }
}
#[derive(Parser)]
#[command(
    name = "sessionctl",
    bin_name = "sessionctl",
    arg_required_else_help = true
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

const fn active_name(s: session_core_v1::ActiveState) -> &'static str {
    match s {
        session_core_v1::ActiveState::Online => "online",
        session_core_v1::ActiveState::Active => "active",
        session_core_v1::ActiveState::Closing => "closing",
        session_core_v1::ActiveState::Opening => "opening",
    }
}
const fn class_name(s: session_core_v1::SessionClass) -> &'static str {
    match s {
        session_core_v1::SessionClass::User => "user",
        session_core_v1::SessionClass::Greeter => "greeter",
        session_core_v1::SessionClass::LockScreen => "lock-screen",
        session_core_v1::SessionClass::Background => "background",
        session_core_v1::SessionClass::BackgroundLight => "background-light",
        session_core_v1::SessionClass::Manager => "manager",
        session_core_v1::SessionClass::ManagerEarly => "manager-early",
    }
}

const fn user_state_name(s: session_core_v1::UserState) -> &'static str {
    match s {
        session_core_v1::UserState::Offline => "offline",
        session_core_v1::UserState::Lingering => "lingering",
        session_core_v1::UserState::Online => "online",
        session_core_v1::UserState::Active => "active",
        session_core_v1::UserState::Closing => "closing",
    }
}

const fn seat_mode_name(mode: seat_core_v1::SeatMode) -> &'static str {
    match mode {
        seat_core_v1::SeatMode::VtBound => "vt-bound",
        seat_core_v1::SeatMode::NonVtBound => "non-vt-bound",
    }
}

const fn seat_client_state_name(state: seat_core_v1::ClientState) -> &'static str {
    match state {
        seat_core_v1::ClientState::New => "new",
        seat_core_v1::ClientState::Enabled => "enabled",
        seat_core_v1::ClientState::PendingDisabled => "pending-disabled",
        seat_core_v1::ClientState::Disabled => "disabled",
        seat_core_v1::ClientState::Closed => "closed",
    }
}

const fn seat_device_kind_name(kind: seat_core_v1::DeviceKind) -> &'static str {
    match kind {
        seat_core_v1::DeviceKind::Drm => "drm",
        seat_core_v1::DeviceKind::Evdev => "evdev",
        seat_core_v1::DeviceKind::Hidraw => "hidraw",
        seat_core_v1::DeviceKind::Wscons => "wscons",
        seat_core_v1::DeviceKind::Unknown => "unknown",
    }
}

fn process_name(pid: u32) -> Option<String> {
    let name = fs::read_to_string(format!("/proc/{pid}/comm")).ok()?;
    let name = name.trim();
    (!name.is_empty()).then(|| name.to_owned())
}

fn select_seat_name(requested: Option<&str>, names: &[String]) -> anyhow::Result<String> {
    if let Some(requested) = requested {
        anyhow::ensure!(
            names.iter().any(|name| name == requested),
            "unknown seat: {requested}"
        );
        return Ok(requested.to_owned());
    }

    match names {
        [] => anyhow::bail!("no seats configured"),
        [name] => Ok(name.clone()),
        _ => anyhow::bail!("multiple seats found; specify one of: {}", names.join(", ")),
    }
}

fn format_since(timestamp: u32) -> String {
    let Some(start) = time::UNIX_EPOCH.checked_add(time::Duration::from_secs(u64::from(timestamp)))
    else {
        return String::from("?");
    };
    let Ok(elapsed) = start.elapsed() else {
        return String::from("?");
    };

    let seconds = elapsed.as_secs();
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let seconds = seconds % 60;
    if hours > 0 {
        format!("{hours}h {minutes}min ago")
    } else if minutes > 0 {
        format!("{minutes}min {seconds}s ago")
    } else {
        format!("{seconds}s ago")
    }
}

fn new_table() -> comfy_table::Table {
    let mut table = comfy_table::Table::new();
    table.load_preset(presets::NOTHING);
    table
        .set_style(comfy_table::TableComponent::LeftHeaderIntersection, '─')
        .set_style(comfy_table::TableComponent::HeaderLines, '─')
        .set_style(comfy_table::TableComponent::MiddleHeaderIntersections, '─')
        .set_style(comfy_table::TableComponent::RightHeaderIntersection, '─')
        .set_style(comfy_table::TableComponent::VerticalLines, '│');
    table
}

fn json(v: &serde_json::Value) -> anyhow::Result<()> {
    println!("{}", serde_json::to_string_pretty(&v)?);
    Ok(())
}

fn cli_command(seat_available: bool) -> clap::Command {
    let command = Cli::command();
    if seat_available {
        command
    } else {
        command.mut_subcommand("seat", |command| command.hide(true))
    }
}

fn load_seat_state(mut socket: client::Client) -> anyhow::Result<SeatState> {
    let mut state = SeatState::default();
    let queue = socket.new_event_queue();

    socket.add_implementation::<seat_core_v1::SeatCoreV1>();
    queue.wait_for_handshake(&mut state)?;
    let _manager = socket.bind::<seat_core_manager::SeatCoreManager, _>(&queue, &mut state, 1)?;
    queue.roundtrip(&mut state)?;

    while !state.initial_dump_done() && state.error.is_none() {
        queue.dispatch_events(&mut state, true)?;
    }
    if let Some(error) = state.error.take() {
        anyhow::bail!("seat daemon: {error}");
    }

    Ok(state)
}

fn seat_for_session(state: Option<&SeatState>, session_id: u32) -> Option<&str> {
    state?
        .clients
        .iter()
        .find(|client| client.snapshot != SnapshotState::Destroyed && client.session == session_id)
        .map(|client| client.seat.as_str())
}

fn run_seat(command: &SeatCommand, socket: client::Client) -> anyhow::Result<()> {
    let state = load_seat_state(socket)?;

    let mut seats = state
        .seats
        .iter()
        .filter(|seat| seat.snapshot != SnapshotState::Destroyed)
        .collect::<Vec<_>>();
    seats.sort_by(|left, right| left.name.cmp(&right.name));

    match command {
        SeatCommand::List { json: output_json } => {
            if *output_json {
                return json(&json!({
                    "seats": seats.iter().map(|seat| json!({
                        "name": seat.name,
                        "mode": seat_mode_name(seat.mode),
                        "current_vt": seat.current_vt,
                        "clients": seat.clients.len(),
                    })).collect::<Vec<_>>(),
                }));
            }

            if seats.is_empty() {
                println!("No seats configured.");
                return Ok(());
            }

            let mut table = new_table();
            table.set_header(["SEAT", "MODE", "VT", "CLIENTS"]);
            for seat in seats {
                table.add_row([
                    seat.name.clone(),
                    seat_mode_name(seat.mode).to_owned(),
                    if seat.current_vt == 0 {
                        "-".to_owned()
                    } else {
                        seat.current_vt.to_string()
                    },
                    seat.clients.len().to_string(),
                ]);
            }
            println!("{table}");
        }
        SeatCommand::Status {
            seat: requested,
            json: output_json,
        } => {
            let names = seats
                .iter()
                .map(|seat| seat.name.clone())
                .collect::<Vec<_>>();
            let seat_name = select_seat_name(requested.as_deref(), &names)?;
            let seat = seats
                .into_iter()
                .find(|seat| seat.name == seat_name)
                .ok_or_else(|| anyhow::anyhow!("unknown seat: {seat_name}"))?;

            let mut clients = state
                .clients
                .iter()
                .filter(|client| {
                    client.seat == seat.name && client.snapshot != SnapshotState::Destroyed
                })
                .collect::<Vec<_>>();
            clients.sort_by_key(|client| client.pid);

            if *output_json {
                return json(&json!({
                    "seat": {
                        "name": seat.name,
                        "mode": seat_mode_name(seat.mode),
                        "current_vt": seat.current_vt,
                        "clients": clients.iter().map(|client| json!({
                            "pid": client.pid,
                            "process": process_name(client.pid),
                            "session": client.session,
                            "state": seat_client_state_name(client.state),
                            "active": client.active,
                            "next": client.next,
                            "uid": client.uid,
                            "gid": client.gid,
                            "devices": state.devices.iter()
                                .filter(|device| {
                                    device.seat == seat.name
                                        && device.client_pid == client.pid
                                        && device.snapshot != SnapshotState::Destroyed
                                })
                                .map(|device| json!({
                                    "id": device.id,
                                    "path": device.path,
                                    "kind": seat_device_kind_name(device.kind),
                                    "active": device.active,
                                }))
                                .collect::<Vec<_>>(),
                        })).collect::<Vec<_>>(),
                    }
                }));
            }

            let vt = if seat.current_vt == 0 {
                "-".to_owned()
            } else {
                seat.current_vt.to_string()
            };
            let mut tree = termtree::Tree::new(format!(
                "seat {}  mode={}  vt={vt}",
                seat.name,
                seat_mode_name(seat.mode)
            ));

            if clients.is_empty() {
                tree.push("no clients available".to_owned());
                println!("{tree}");
                return Ok(());
            }

            for client in &clients {
                let mut flags = Vec::new();
                if client.active {
                    flags.push("active");
                }
                if client.next {
                    flags.push("next");
                }
                let flags = if flags.is_empty() {
                    String::new()
                } else {
                    format!(" [{}]", flags.join(", "))
                };
                let process = process_name(client.pid)
                    .map(|name| format!(" ({name})"))
                    .unwrap_or_default();
                let mut client_tree = termtree::Tree::new(format!(
                    "client pid={}{}  session={}  uid={}  gid={}  state={}{}",
                    client.pid,
                    process,
                    client.session,
                    client.uid,
                    client.gid,
                    seat_client_state_name(client.state),
                    flags
                ));

                let mut devices = state
                    .devices
                    .iter()
                    .filter(|device| {
                        device.seat == seat.name
                            && device.client_pid == client.pid
                            && device.snapshot != SnapshotState::Destroyed
                    })
                    .collect::<Vec<_>>();
                devices.sort_by_key(|device| device.id);
                if devices.is_empty() {
                    client_tree.push("no devices available".to_owned());
                } else {
                    for device in devices {
                        client_tree.push(format!(
                            "device id={}  kind={}  state={}  path={}",
                            device.id,
                            seat_device_kind_name(device.kind),
                            if device.active { "active" } else { "inactive" },
                            device.path
                        ));
                    }
                }
                tree.push(client_tree);
            }
            println!("{tree}");
        }
    }

    Ok(())
}

fn main() -> anyhow::Result<()> {
    let seat_connection = client::Client::connect("/run/sessiond-seat.sock").ok();
    let mut command = cli_command(seat_connection.is_some());
    let matches = command.clone().get_matches();
    let cli = Cli::from_arg_matches(&matches)?;
    if let Command::Completions { shell } = cli.command {
        shell.generate(&mut command);
        return Ok(());
    }
    if let Command::Seat { command } = &cli.command {
        let Some(socket) = seat_connection else {
            anyhow::bail!("seat daemon is unavailable at /run/sessiond-seat.sock");
        };
        return run_seat(command, socket);
    }

    let mut state = State::default();
    let mut socket = client::Client::connect("/run/sessiond/sessiond.sock")?;
    let queue = socket.new_event_queue();

    socket.add_implementation::<session_core_v1::SessionCoreV1>();
    socket.add_implementation::<session_power_v1::SessionPowerV1>();

    queue.wait_for_handshake(&mut state)?;

    let manager =
        socket.bind::<session_core_manager::SessionCoreManager, _>(&queue, &mut state, 2)?;
    let power_manager =
        socket.bind::<session_power_manager::SessionPowerManager, _>(&queue, &mut state, 1)?;

    queue.roundtrip(&mut state)?;

    while !state.done {
        queue.dispatch_events(&mut state, true)?;
    }

    if let Some(error) = state.error.take() {
        anyhow::bail!("{error}");
    }
    let ids: Vec<_> = state.sessions.iter().map(|s| s.id).collect();
    for id in ids {
        state.session(&manager, id);
    }
    while state.sessions.iter().any(|s| !s.done) {
        queue.dispatch_events(&mut state, true)?;
    }

    if matches!(&cli.command, Command::User { .. }) {
        while state.users.iter().any(|u| !u.done) {
            queue.dispatch_events(&mut state, true)?;
        }
    }

    let seat_state = if matches!(
        &cli.command,
        Command::Session {
            command: SessionCommand::List { .. }
        }
    ) {
        seat_connection.and_then(|socket| load_seat_state(socket).ok())
    } else {
        None
    };

    match cli.command {
        Command::Session {
            command: SessionCommand::List { json: output_json },
        } => {
            if output_json {
                return json(&json!({
                    "sessions": state.sessions.iter()
                        .map(|session| {
                            let mut value = json!({
                                "id": session.id,
                                "uid": session.uid,
                                "username": session.username,
                                "class": class_name(session.class),
                                "vt": session.vt,
                                "tty": session.vt.checked_sub(1).map(|_| format!("tty{}", session.vt)),
                                "leader": session.leader,
                                "timestamp": session.timestamp,
                                "active": active_name(session.active),
                                "can_modify": session.can_modify,
                            });
                            if let Some((seat_state, object)) = seat_state.as_ref().zip(value.as_object_mut()) {
                                object.insert(
                                    "seat".to_owned(),
                                    json!(seat_for_session(Some(seat_state), session.id)),
                                );
                            }
                            value
                        })
                        .collect::<Vec<_>>(),
                }));
            }

            if state.sessions.is_empty() {
                println!("No active sessions.");
                return Ok(());
            }

            let mut table = new_table();
            let mut header = vec!["ID", "UID", "USER", "CLASS", "TTY"];
            if seat_state.is_some() {
                header.push("SEAT");
            }
            header.extend(["STATE", "SINCE"]);
            table.set_header(header);
            for s in state.sessions {
                let mut row = vec![
                    s.id.to_string(),
                    s.uid.to_string(),
                    s.username,
                    class_name(s.class).to_owned(),
                    if s.vt == 0 {
                        "-".to_owned()
                    } else {
                        format!("tty{}", s.vt)
                    },
                ];
                if let Some(seat_state) = seat_state.as_ref() {
                    row.push(
                        seat_for_session(Some(seat_state), s.id)
                            .unwrap_or("-")
                            .to_owned(),
                    );
                }
                row.extend([active_name(s.active).to_owned(), format_since(s.timestamp)]);
                table.add_row(row);
            }
            println!("{table}");
        }
        Command::Session {
            command: SessionCommand::Close { ids },
        } => {
            for id in ids {
                if let Some(s) = state.sessions.iter().find(|s| s.id == id)
                    && let Some(o) = &s.object
                {
                    o.send_terminate();
                }
            }
        }
        Command::Session {
            command: SessionCommand::Kill { ids },
        } => {
            for id in ids {
                if let Some(s) = state.sessions.iter().find(|s| s.id == id)
                    && let Some(o) = &s.object
                {
                    o.send_kill();
                }
            }
        }
        Command::User {
            command: UserCommand::List { json: output_json },
        } => {
            if output_json {
                return json(
                    &json!({"users": state.users.iter().map(|u| json!({"uid":u.uid,"username":u.username,"state":user_state_name(u.state),"lingering":u.lingering})).collect::<Vec<_>>() }),
                );
            }
            if state.users.is_empty() {
                return Ok(());
            }
            let mut table = new_table();
            table.set_header(["UID", "USER", "STATE", "LINGERING"]);
            for u in state.users {
                table.add_row([
                    u.uid.to_string(),
                    u.username,
                    user_state_name(u.state).to_owned(),
                    u.lingering.to_string(),
                ]);
            }
            println!("{table}");
        }
        Command::User {
            command: UserCommand::Terminate { uids },
        } => {
            for uid in uids {
                if let Some(u) = state.users.iter().find(|u| u.uid == uid)
                    && let Some(o) = &u.object
                {
                    o.send_terminate();
                }
            }
        }
        Command::User {
            command: UserCommand::Kill { uids },
        } => {
            for uid in uids {
                if let Some(u) = state.users.iter().find(|u| u.uid == uid)
                    && let Some(o) = &u.object
                {
                    o.send_kill();
                }
            }
        }
        Command::Reboot | Command::Poweroff | Command::Suspend | Command::Hibernate => {
            while state.can_control_power.is_none() {
                queue.dispatch_events(&mut state, true)?;
            }
            anyhow::ensure!(
                state.can_control_power == Some(true),
                "power operation is not permitted while other users have active sessions"
            );
            match cli.command {
                Command::Reboot => power_manager.send_reboot(),
                Command::Poweroff => power_manager.send_poweroff(),
                Command::Suspend => power_manager.send_suspend(),
                Command::Hibernate => power_manager.send_hibernate(),
                _ => unreachable!(),
            }
        }
        Command::Completions { .. } | Command::Seat { .. } => unreachable!(),
    }
    Ok(())
}
