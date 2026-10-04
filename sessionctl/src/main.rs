use anyhow::Context;
use clap::{CommandFactory, FromArgMatches, Parser, Subcommand, ValueEnum};
use comfy_table::presets;
use hyprwire::client;
use hyprwire_protocols_sessiond::client::session_core::v1::session_core_v1;
use hyprwire_protocols_sessiond::client::session_core::v1::session_core_v1::{
    session_core_inhibitor, session_core_manager, session_core_session, session_core_user,
};
use hyprwire_protocols_sessiond::client::session_inhibit::v1::session_inhibit_v1;
use hyprwire_protocols_sessiond::client::session_inhibit::v1::session_inhibit_v1::{
    session_inhibit_manager, session_inhibitor,
};
use owo_colors::OwoColorize;
use serde_json::json;
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::{env, fs, io, path, process, time};

struct Session {
    object: Option<session_core_session::SessionCoreSession>,
    id: u32,
    uid: u32,
    username: String,
    class: session_core_v1::SessionClass,
    vt: u32,
    leader: u32,
    timestamp: u32,
    state: session_core_v1::SessionState,
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

struct Inhibitor {
    object: Option<session_core_inhibitor::SessionCoreInhibitor>,
    id: u32,
    who: String,
    uid: u32,
    username: String,
    pid: u32,
    comm: String,
    what: u32,
    why: String,
    mode: session_core_v1::InhibitorMode,
    done: bool,
}

#[derive(Default)]
struct InhibitState {
    active: bool,
    error: Option<String>,
}

impl hyprwire::Dispatch<session_inhibit_manager::SessionInhibitManager> for InhibitState {
    fn event(
        &mut self,
        _object: &session_inhibit_manager::SessionInhibitManager,
        _event: <session_inhibit_manager::SessionInhibitManager as hyprwire::Object>::Event<'_>,
    ) {
    }
}

impl hyprwire::Dispatch<session_inhibitor::SessionInhibitor> for InhibitState {
    fn event(
        &mut self,
        _object: &session_inhibitor::SessionInhibitor,
        event: <session_inhibitor::SessionInhibitor as hyprwire::Object>::Event<'_>,
    ) {
        match event {
            session_inhibitor::Event::Active => self.active = true,
            session_inhibitor::Event::Failed => {
                self.error = Some("what is zero or contains unknown bits".to_string());
            }
            _ => {}
        }
    }
}

#[derive(Default)]
struct State {
    sessions: Vec<Session>,
    users: Vec<User>,
    inhibitors: Vec<Inhibitor>,
    done: bool,
    error: Option<String>,
}

impl State {
    fn session(&mut self, manager: &session_core_manager::SessionCoreManager, id: u32) {
        if self.sessions.iter().any(|s| s.id == id) {
            return;
        }
        if let Some(object) = manager.send_get_session_by_id::<Self>(id) {
            let s = Session {
                object: Some(object),
                id,
                uid: 0,
                username: String::new(),
                class: session_core_v1::SessionClass::User,
                vt: 0,
                leader: 0,
                timestamp: 0,
                state: session_core_v1::SessionState::Online,
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

    fn inhibitor(&mut self, manager: &session_core_manager::SessionCoreManager, id: u32) {
        if self.inhibitors.iter().any(|inhibitor| inhibitor.id == id) {
            return;
        }
        if let Some(object) = manager.send_get_inhibitor::<Self>(id) {
            self.inhibitors.push(Inhibitor {
                object: Some(object),
                id,
                who: String::new(),
                uid: 0,
                username: String::new(),
                pid: 0,
                comm: String::new(),
                what: 0,
                why: String::new(),
                mode: session_core_v1::InhibitorMode::Block,
                done: false,
            });
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
            session_core_manager::Event::InhibitorAdded { inhibitor_id } => {
                self.inhibitor(object, inhibitor_id);
                self.done = false;
            }
            session_core_manager::Event::InhibitorRemoved { inhibitor_id } => {
                self.inhibitors
                    .retain(|inhibitor| inhibitor.id != inhibitor_id);
            }
            session_core_manager::Event::Done => self.done = true,
            _ => {}
        }
    }
}
impl hyprwire::Dispatch<session_core_session::SessionCoreSession> for State {
    fn event(
        &mut self,
        object: &session_core_session::SessionCoreSession,
        event: <session_core_session::SessionCoreSession as hyprwire::Object>::Event<'_>,
    ) {
        let Some(index) = self
            .sessions
            .iter()
            .position(|s| s.object.as_ref() == Some(object))
        else {
            return;
        };
        let s = &mut self.sessions[index];
        match event {
            session_core_session::Event::SessionId { session_id } => s.id = session_id,
            session_core_session::Event::Uid { uid } => {
                s.uid = uid;
                s.username = uzers::get_user_by_uid(uid)
                    .map(|user| user.name().to_string_lossy().into_owned())
                    .unwrap_or_default();
            }
            session_core_session::Event::Class { class } => s.class = class,
            session_core_session::Event::Vt { vt } => s.vt = vt,
            session_core_session::Event::Leader { leader } => s.leader = leader,
            session_core_session::Event::Timestamp { timestamp } => s.timestamp = timestamp,
            session_core_session::Event::State { state } => s.state = state,
            session_core_session::Event::CanModify { can_modify } => s.can_modify = can_modify != 0,
            session_core_session::Event::Done => s.done = true,
            session_core_session::Event::Destroyed => {
                self.sessions.remove(index);
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
            session_core_user::Event::Uid { uid } => {
                u.uid = uid;
                u.username = uzers::get_user_by_uid(uid)
                    .map(|user| user.name().to_string_lossy().into_owned())
                    .unwrap_or_default();
            }
            session_core_user::Event::Lingering { lingering } => u.lingering = lingering != 0,
            session_core_user::Event::State { state } => u.state = state,
            session_core_user::Event::CanModify { can_modify } => u.can_modify = can_modify != 0,
            session_core_user::Event::Done | session_core_user::Event::Failed => u.done = true,
            _ => {}
        }
    }
}

impl hyprwire::Dispatch<session_core_inhibitor::SessionCoreInhibitor> for State {
    fn event(
        &mut self,
        object: &session_core_inhibitor::SessionCoreInhibitor,
        event: <session_core_inhibitor::SessionCoreInhibitor as hyprwire::Object>::Event<'_>,
    ) {
        let Some(index) = self
            .inhibitors
            .iter()
            .position(|inhibitor| inhibitor.object.as_ref() == Some(object))
        else {
            return;
        };

        match event {
            session_core_inhibitor::Event::InhibitorId { inhibitor_id } => {
                self.inhibitors[index].id = inhibitor_id;
            }
            session_core_inhibitor::Event::Who { who } => self.inhibitors[index].who = who,
            session_core_inhibitor::Event::Uid { uid } => {
                self.inhibitors[index].uid = uid;
                self.inhibitors[index].username = uzers::get_user_by_uid(uid)
                    .map(|user| user.name().to_string_lossy().into_owned())
                    .unwrap_or_default();
            }
            session_core_inhibitor::Event::Pid { pid } => self.inhibitors[index].pid = pid,
            session_core_inhibitor::Event::Comm { comm } => self.inhibitors[index].comm = comm,
            session_core_inhibitor::Event::What { what } => self.inhibitors[index].what = what,
            session_core_inhibitor::Event::Why { why } => self.inhibitors[index].why = why,
            session_core_inhibitor::Event::Mode { mode } => self.inhibitors[index].mode = mode,
            session_core_inhibitor::Event::Done => self.inhibitors[index].done = true,
            session_core_inhibitor::Event::Destroyed => {
                self.inhibitors.remove(index);
            }
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
enum InhibitorCommand {
    List {
        #[arg(long)]
        json: bool,
    },
    Acquire {
        #[arg(long, required = true, value_delimiter = ':')]
        what: Vec<InhibitWhat>,
        #[arg(long, default_value = "sessionctl")]
        who: String,
        #[arg(long)]
        why: String,
        #[arg(long, default_value = "block")]
        mode: InhibitMode,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum InhibitWhat {
    Shutdown,
    Sleep,
    Idle,
    HandlePowerKey,
    HandleSuspendKey,
    HandleHibernateKey,
    HandleLidSwitch,
    HandleRebootKey,
}

impl InhibitWhat {
    const fn bit(self) -> u32 {
        match self {
            Self::Shutdown => session_inhibit_v1::What::Shutdown as u32,
            Self::Sleep => session_inhibit_v1::What::Sleep as u32,
            Self::Idle => session_inhibit_v1::What::Idle as u32,
            Self::HandlePowerKey => session_inhibit_v1::What::HandlePowerKey as u32,
            Self::HandleSuspendKey => session_inhibit_v1::What::HandleSuspendKey as u32,
            Self::HandleHibernateKey => session_inhibit_v1::What::HandleHibernateKey as u32,
            Self::HandleLidSwitch => session_inhibit_v1::What::HandleLidSwitch as u32,
            Self::HandleRebootKey => session_inhibit_v1::What::HandleRebootKey as u32,
        }
    }
}

#[derive(Clone, Copy, ValueEnum)]
enum InhibitMode {
    Block,
    BlockWeak,
    Delay,
}

impl From<InhibitMode> for session_inhibit_v1::Mode {
    fn from(mode: InhibitMode) -> Self {
        match mode {
            InhibitMode::Block => Self::Block,
            InhibitMode::BlockWeak => Self::BlockWeak,
            InhibitMode::Delay => Self::Delay,
        }
    }
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
    Inhibitor {
        #[command(subcommand)]
        command: InhibitorCommand,
    },
    Completions {
        shell: CompletionShell,
    },
    #[command(external_subcommand)]
    External(Vec<OsString>),
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

const fn session_state_name(state: session_core_v1::SessionState) -> &'static str {
    match state {
        session_core_v1::SessionState::Online => "online",
        session_core_v1::SessionState::Active => "active",
        session_core_v1::SessionState::Closing => "closing",
        session_core_v1::SessionState::Opening => "opening",
        _ => "unknown",
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
        _ => "unknown",
    }
}

const fn user_state_name(s: session_core_v1::UserState) -> &'static str {
    match s {
        session_core_v1::UserState::Offline => "offline",
        session_core_v1::UserState::Lingering => "lingering",
        session_core_v1::UserState::Online => "online",
        session_core_v1::UserState::Active => "active",
        session_core_v1::UserState::Closing => "closing",
        _ => "unknown",
    }
}

fn inhibitor_what_name(what: u32) -> String {
    const FLAGS: [(u32, &str); 8] = [
        (1, "shutdown"),
        (2, "sleep"),
        (4, "idle"),
        (8, "handle-power-key"),
        (16, "handle-suspend-key"),
        (32, "handle-hibernate-key"),
        (64, "handle-lid-switch"),
        (128, "handle-reboot-key"),
    ];

    FLAGS
        .into_iter()
        .filter_map(|(flag, name)| (what & flag != 0).then_some(name))
        .collect::<Vec<_>>()
        .join(":")
}

const fn inhibitor_mode_name(mode: session_core_v1::InhibitorMode) -> &'static str {
    match mode {
        session_core_v1::InhibitorMode::Block => "block",
        session_core_v1::InhibitorMode::BlockWeak => "block-weak",
        session_core_v1::InhibitorMode::Delay => "delay",
        _ => "unknown",
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

fn external_command_names() -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for directory in env::var_os("PATH")
        .into_iter()
        .flat_map(|path| env::split_paths(&path).collect::<Vec<_>>())
    {
        let Ok(entries) = fs::read_dir(directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let file_name = entry.file_name();
            let Some(file_name) = file_name.to_str() else {
                continue;
            };
            let Some(name) = file_name
                .strip_prefix("sessiond-")
                .and_then(|name| name.strip_suffix("ctl"))
                .filter(|name| !name.is_empty())
            else {
                continue;
            };
            if external_command_path(name).is_some() {
                names.insert(name.to_owned());
            }
        }
    }
    names
}

fn external_command_path(name: &str) -> Option<path::PathBuf> {
    if name.is_empty() || name.contains('/') || name.contains('\\') {
        return None;
    }
    let executable = format!("sessiond-{name}ctl");
    env::var_os("PATH")
        .into_iter()
        .flat_map(|path| env::split_paths(&path).collect::<Vec<_>>())
        .map(|directory| directory.join(&executable))
        .find(|path| {
            path.is_file() && {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    path.metadata()
                        .is_ok_and(|metadata| metadata.permissions().mode() & 0o111 != 0)
                }
                #[cfg(not(unix))]
                {
                    true
                }
            }
        })
}

fn run_external_command(args: Vec<OsString>) -> anyhow::Result<()> {
    let Some((name, forwarded_args)) = args.split_first() else {
        anyhow::bail!("missing external subcommand name");
    };
    let name = name
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("external subcommand name is not valid UTF-8"))?;
    let Some(path) = external_command_path(name) else {
        anyhow::bail!("unknown subcommand: {name}");
    };

    let status = process::Command::new(path).args(forwarded_args).status()?;
    process::exit(status.code().unwrap_or(1));
}

fn acquire_inhibitor(
    what: &[InhibitWhat],
    who: &str,
    why: &str,
    mode: InhibitMode,
) -> anyhow::Result<()> {
    let mut state = InhibitState::default();
    let mut socket = client::Client::connect("/run/sessiond/sessiond.sock")
        .context("failed to connect to /run/sessiond/sessiond.sock")?;
    let queue = socket.new_event_queue();

    socket.add_implementation::<session_inhibit_v1::SessionInhibitV1>();
    queue.wait_for_handshake(&mut state)?;
    let manager =
        socket.bind::<session_inhibit_manager::SessionInhibitManager, _>(&queue, &mut state, 1)?;
    let what = what.iter().fold(0, |mask, value| mask | value.bit());
    let _inhibitor = manager
        .send_inhibit::<InhibitState>(what, who, why, mode.into())
        .ok_or_else(|| anyhow::anyhow!("failed to create inhibitor object"))?;

    while !state.active && state.error.is_none() {
        queue.dispatch_events(&mut state, true)?;
    }
    if let Some(error) = state.error.take() {
        anyhow::bail!("failed to acquire inhibitor: {error}");
    }

    eprintln!("Inhibitor active; press Ctrl-C to release it.");
    loop {
        queue.dispatch_events(&mut state, true)?;
        if let Some(error) = state.error.take() {
            anyhow::bail!("inhibitor error: {error}");
        }
    }
}

fn main() -> anyhow::Result<()> {
    let mut command = Cli::command();
    let external_commands = external_command_names();
    if !external_commands.is_empty() {
        let names = external_commands
            .iter()
            .map(|name| format!("  {}  external command", name.bold()))
            .collect::<Vec<_>>()
            .join("\n");
        command = command.after_help(format!(
            "\n{}\n{names}",
            "External commands".bold().underline()
        ));
    }
    let matches = command.clone().get_matches();
    let cli = Cli::from_arg_matches(&matches)?;
    if let Command::External(args) = &cli.command {
        return run_external_command(args.clone());
    }
    if let Command::Completions { shell } = cli.command {
        shell.generate(&mut command);
        return Ok(());
    }
    if let Command::Inhibitor {
        command:
            InhibitorCommand::Acquire {
                what,
                who,
                why,
                mode,
            },
    } = &cli.command
    {
        return acquire_inhibitor(what, who, why, *mode);
    }

    let mut state = State::default();
    let mut socket = client::Client::connect("/run/sessiond/sessiond.sock")
        .context("failed to connect to /run/sessiond/sessiond.sock")?;
    let queue = socket.new_event_queue();

    socket.add_implementation::<session_core_v1::SessionCoreV1>();

    queue.wait_for_handshake(&mut state)?;

    let manager =
        socket.bind::<session_core_manager::SessionCoreManager, _>(&queue, &mut state, 1)?;

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

    if matches!(&cli.command, Command::Inhibitor { .. }) {
        while state.inhibitors.iter().any(|inhibitor| !inhibitor.done) {
            queue.dispatch_events(&mut state, true)?;
        }
    }

    match cli.command {
        Command::Session {
            command: SessionCommand::List { json: output_json },
        } => {
            if output_json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "sessions": state.sessions.iter()
                            .map(|session| {
                                json!({
                                    "id": session.id,
                                    "uid": session.uid,
                                    "username": session.username,
                                    "class": class_name(session.class),
                                    "vt": session.vt,
                                    "tty": session.vt.checked_sub(1).map(|_| format!("tty{}", session.vt)),
                                    "leader": session.leader,
                                    "timestamp": session.timestamp,
                                    "state": session_state_name(session.state),
                                    "can_modify": session.can_modify,
                                })
                            })
                            .collect::<Vec<_>>(),
                    }))?
                );
                return Ok(());
            }

            if state.sessions.is_empty() {
                println!("No active sessions.");
                return Ok(());
            }

            let mut table = new_table();
            table.set_header(["ID", "UID", "USER", "CLASS", "TTY", "STATE", "SINCE"]);
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
                row.extend([
                    session_state_name(s.state).to_owned(),
                    format_since(s.timestamp),
                ]);
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
                println!(
                    "{}",
                    serde_json::to_string_pretty(
                        &json!({"users": state.users.iter().map(|u| json!({"uid":u.uid,"username":u.username,"state":user_state_name(u.state),"lingering":u.lingering})).collect::<Vec<_>>() })
                    )?
                );
                return Ok(());
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
        Command::Inhibitor {
            command: InhibitorCommand::List { json: output_json },
        } => {
            if output_json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "inhibitors": state.inhibitors.iter().map(|inhibitor| json!({
                            "who": inhibitor.who,
                            "uid": inhibitor.uid,
                            "username": inhibitor.username,
                            "pid": inhibitor.pid,
                            "comm": inhibitor.comm,
                            "what": inhibitor_what_name(inhibitor.what),
                            "why": inhibitor.why,
                            "mode": inhibitor_mode_name(inhibitor.mode),
                        })).collect::<Vec<_>>(),
                    }))?
                );
                return Ok(());
            }

            if state.inhibitors.is_empty() {
                println!("No inhibitors.");
                return Ok(());
            }

            let mut table = new_table();
            table.set_header(["WHO", "UID", "USER", "PID", "COMM", "WHAT", "WHY", "MODE"]);
            for inhibitor in state.inhibitors {
                table.add_row([
                    inhibitor.who,
                    inhibitor.uid.to_string(),
                    inhibitor.username,
                    inhibitor.pid.to_string(),
                    inhibitor.comm,
                    inhibitor_what_name(inhibitor.what),
                    inhibitor.why,
                    inhibitor_mode_name(inhibitor.mode).to_owned(),
                ]);
            }
            println!("{table}");
        }
        Command::Inhibitor {
            command: InhibitorCommand::Acquire { .. },
        }
        | Command::Completions { .. }
        | Command::External(_) => unreachable!(),
    }
    Ok(())
}
