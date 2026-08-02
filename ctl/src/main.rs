mod session_core_v1 {
    hyprwire::include_protocol!("session_core_v1");
    pub use client::*;
    pub use spec::*;
}

use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use clap_complete::{Shell, generate};
use clap_complete_nushell::Nushell;
use hyprwire::client;
use serde_json::json;
use session_core_v1::{session_core_manager, session_core_session, session_core_user};
use std::io;

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
#[derive(Default)]
struct State {
    sessions: Vec<Session>,
    users: Vec<User>,
    done: bool,
    can_control_power: bool,
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
            session_core_manager::Event::Done => self.done = true,
            session_core_manager::Event::CanControlPower { can_control_power } => {
                self.can_control_power = can_control_power != 0;
            }
            session_core_manager::Event::Error { message, .. } => {
                self.error = Some(message);
                self.done = true;
            }
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
enum Command {
    Session {
        #[command(subcommand)]
        command: SessionCommand,
    },
    User {
        #[command(subcommand)]
        command: UserCommand,
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
            Self::Bash => generate(Shell::Bash, command, "sessionctl", &mut stdout),
            Self::Elvish => generate(Shell::Elvish, command, "sessionctl", &mut stdout),
            Self::Fish => generate(Shell::Fish, command, "sessionctl", &mut stdout),
            Self::Nushell => generate(Nushell, command, "sessionctl", &mut stdout),
            Self::Powershell => generate(Shell::PowerShell, command, "sessionctl", &mut stdout),
            Self::Zsh => generate(Shell::Zsh, command, "sessionctl", &mut stdout),
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

fn json(v: &serde_json::Value) -> anyhow::Result<()> {
    println!("{}", serde_json::to_string_pretty(&v)?);
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    if let Command::Completions { shell } = cli.command {
        shell.generate(&mut Cli::command());
        return Ok(());
    }
    let mut state = State::default();
    let mut socket = client::Client::connect("/run/sessiond.sock")?;
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
    match cli.command {
        Command::Session {
            command: SessionCommand::List { json: output_json },
        } => {
            if output_json {
                return json(
                    &json!({ "sessions": state.sessions.iter().map(|s| json!({"id":s.id,"uid":s.uid,"username":s.username,"class":class_name(s.class),"vt":s.vt,"tty":s.vt.checked_sub(1).map(|_| format!("tty{}", s.vt)),"leader":s.leader,"timestamp":s.timestamp,"active":active_name(s.active),"can_modify":s.can_modify})).collect::<Vec<_>>() }),
                );
            }
            println!("  ID  UID  USER  CLASS  TTY  STATE");
            for s in state.sessions {
                println!(
                    "  {:<3} {:<4} {:<5} {:<6} {:<4} {}",
                    s.id,
                    s.uid,
                    s.username,
                    class_name(s.class),
                    if s.vt == 0 {
                        "-".into()
                    } else {
                        format!("tty{}", s.vt)
                    },
                    active_name(s.active)
                );
            }
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
            let uids: Vec<_> = state.sessions.iter().map(|s| s.uid).collect();
            for uid in uids {
                state.user(&manager, uid);
            }
            while state.users.iter().any(|u| !u.done) {
                queue.dispatch_events(&mut state, true)?;
            }
            if output_json {
                return json(
                    &json!({"users": state.users.iter().map(|u| json!({"uid":u.uid,"username":u.username,"state":user_state_name(u.state),"lingering":u.lingering})).collect::<Vec<_>>() }),
                );
            }
            for u in state.users {
                println!("{} {} {}", u.uid, u.username, user_state_name(u.state));
            }
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
            anyhow::ensure!(
                state.can_control_power,
                "power operation is not permitted while other users have active sessions"
            );
            match cli.command {
                Command::Reboot => manager.send_reboot(),
                Command::Poweroff => manager.send_poweroff(),
                Command::Suspend => manager.send_suspend(),
                Command::Hibernate => manager.send_hibernate(),
                _ => unreachable!(),
            }
        }
        Command::Completions { .. } => unreachable!(),
    }
    Ok(())
}
