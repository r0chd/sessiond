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
use session_core_v1::{session_core_manager, session_core_session};

struct Session {
    id: u32,
    object: session_core_session::SessionCoreSession,
    uid: u32,
    username: String,
    vt: u32,
    seat: Option<String>,
    leader: u32,
    timestamp: u32,
    active: session_core_v1::ActiveState,
    done: bool,
}

struct State {
    sessions: Vec<Session>,
    done: bool,
    error: Option<String>,
}

impl hyprwire::Dispatch<session_core_manager::SessionCoreManager> for State {
    fn event(
        &mut self,
        object: &session_core_manager::SessionCoreManager,
        event: <session_core_manager::SessionCoreManager as hyprwire::Object>::Event<'_>,
    ) {
        match event {
            session_core_manager::Event::Sessions { session_ids } => {
                for session_id in session_ids {
                    self.add_session(object, session_id);
                }
            }
            session_core_manager::Event::SessionAdded { session_id } => {
                self.add_session(object, session_id);
            }
            session_core_manager::Event::SessionRemoved { session_id } => {
                self.sessions.retain(|session| session.id != session_id);
            }
            session_core_manager::Event::Done => {
                self.done = true;
            }
            session_core_manager::Event::Error { code: _, message } => {
                self.error = Some(message);
                self.done = true;
            }
        }
    }
}

impl State {
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
            vt: 0,
            seat: None,
            leader: 0,
            timestamp: 0,
            done: false,
            active: session_core_v1::ActiveState::Online,
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
            _ => {}
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
    #[command(about = "lock sessions")]
    Lock {
        #[arg(value_name = "SESSION", help = "session id; defaults to your session")]
        ids: Vec<u32>,
    },
    #[command(about = "unlock sessions")]
    Unlock {
        #[arg(value_name = "SESSION", help = "session id; defaults to your session")]
        ids: Vec<u32>,
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
            | SessionCommand::Lock { ids }
            | SessionCommand::Unlock { ids } => Some(ids),
        }
    }

    const fn targets_current(command: &SessionCommand) -> bool {
        match command {
            SessionCommand::List { .. } => false,
            SessionCommand::Close { current, .. } | SessionCommand::Kill { current, .. } => {
                *current
            }
            SessionCommand::Lock { ids } | SessionCommand::Unlock { ids } => ids.is_empty(),
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
}

const fn active_state_name(state: session_core_v1::ActiveState) -> &'static str {
    match state {
        session_core_v1::ActiveState::Online => "online",
        session_core_v1::ActiveState::Active => "active",
        session_core_v1::ActiveState::Closing => "closing",
        session_core_v1::ActiveState::Opening => "opening",
    }
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
        done: false,
        error: None,
    };

    let mut socket = client::Client::connect("/run/sessiond.sock")?;
    let event_queue = socket.new_event_queue();

    socket.add_implementation::<session_core_v1::SessionCoreV1Impl>();

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
                if json {
                    return print_json(&json!({
                        "sessions": state.sessions.iter().map(|session| {
                            json!({
                                "id": session.id,
                                "uid": session.uid,
                                "username": session.username,
                                "vt": session.vt,
                                "tty": format!("tty{}", session.vt),
                                "seat": session.seat,
                                "leader": session.leader,
                                "timestamp": session.timestamp,
                                "active": active_state_name(session.active),
                            })
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
                let vt_w = state
                    .sessions
                    .iter()
                    .map(|s| format!("tty{}", s.vt).len())
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
                    "  {:<id_w$}  {:<uid_w$}  {:<user_w$}  {:<vt_w$}  {:<seat_w$}  {:<leader_w$}  {:<since_w$}  {:<state_w$}",
                    "ID", "UID", "USER", "TTY", "SEAT", "LEADER", "SINCE", "STATE",
                );
                println!(
                    "  {}",
                    "─".repeat(
                        id_w + 2
                            + uid_w
                            + 2
                            + user_w
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
                        "  {:<id_w$}  {:<uid_w$}  {:<user_w$}  {:<vt_w$}  {:<seat_w$}  {:<leader_w$}  {:<since_w$}  {:<state_w$}",
                        session.id,
                        session.uid,
                        session.username,
                        format!("tty{}", session.vt),
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
                    action.send(&session.object);

                    return Ok(());
                }

                let mut unmatched = Vec::new();
                let mut matched = Vec::new();

                for id in ids {
                    if let Some(session) = state.sessions.iter().find(|session| session.id == *id) {
                        matched.push(session.object.clone());
                    } else {
                        unmatched.push(id);
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
        Command::Completions { shell: _ } => unreachable!(),
        cmd => {
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
