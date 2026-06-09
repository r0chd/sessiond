mod session_core_v1 {
    hyprwire::include_protocol!("session_core_v1");
    pub use client::*;
    pub use spec::*;
}

use anyhow::Context;
use clap::{Parser, Subcommand};
use hyprwire::client;
use owo_colors::OwoColorize;
use rustix::process;
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
                    let session = Session {
                        id: session_id,
                        object: object.send_get_session::<State>(session_id).unwrap(),
                        uid: 0,
                        username: String::new(),
                        vt: 0,
                        seat: None,
                        leader: 0,
                        timestamp: 0,
                        done: false,
                        active: session_core_v1::ActiveState::Online,
                    };
                    self.sessions.push(session);
                }
            }
            session_core_manager::Event::SessionAdded { session_id } => {
                let session = Session {
                    id: session_id,
                    object: object.send_get_session::<State>(session_id).unwrap(),
                    uid: 0,
                    username: String::new(),
                    vt: 0,
                    seat: None,
                    leader: 0,
                    timestamp: 0,
                    done: false,
                    active: session_core_v1::ActiveState::Online,
                };
                self.sessions.push(session);
            }
            session_core_manager::Event::SessionRemoved { session_id } => {
                self.sessions.retain(|session| session.id != session_id);
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
            session_core_session::Event::Destroyed => {}
            session_core_session::Event::Ok => {
                self.done = true;
            }
            session_core_session::Event::Error { code, message } => {
                self.error = Some(format!("{message} (code={code})"));
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
    #[command(about = "list all active sessions")]
    ListSessions,
    #[command(about = "terminate one or more sessions")]
    TerminateSession { ids: Vec<u32> },
    #[command(about = "kill one or more sessions")]
    KillSession { ids: Vec<u32> },
    #[command(about = "lock one or more sessions")]
    LockSession { ids: Vec<u32> },
    #[command(about = "unlock one or more sessions")]
    UnlockSession { ids: Vec<u32> },
}

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub(crate) struct Cli {
    #[command(subcommand)]
    command: Command,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let mut state = State {
        sessions: Vec::new(),
        done: false,
        error: None,
    };

    let mut socket = client::Client::connect("/run/sessiond.sock")?;
    let event_queue = socket.new_event_queue();

    socket.add_implementation::<session_core_v1::SessionCoreV1Impl>();

    event_queue.wait_for_handshake(&mut state)?;

    let _manager =
        socket.bind::<session_core_manager::SessionCoreManager, _>(&event_queue, &mut state, 1)?;

    event_queue.roundtrip(&mut state)?;

    while state.sessions.iter().any(|session| !session.done) {
        event_queue.dispatch_events(&mut state, true)?;
    }

    match cli.command {
        Command::ListSessions => {
            if state.sessions.is_empty() {
                println!("No active sessions.");
                return Ok(());
            }

            let fmt_state = |s: &session_core_v1::ActiveState| -> &'static str {
                match s {
                    session_core_v1::ActiveState::Online => "online",
                    session_core_v1::ActiveState::Active => "active",
                    session_core_v1::ActiveState::Closing => "closing",
                    session_core_v1::ActiveState::Opening => "opening",
                }
            };

            let fmt_ts = |ts: u32| -> String {
                use std::time::{Duration, UNIX_EPOCH};
                let Some(t) = UNIX_EPOCH.checked_add(Duration::from_secs(ts as u64)) else {
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
                    format!("{}h {}min ago", h, m)
                } else if m > 0 {
                    format!("{}min {}s ago", m, s)
                } else {
                    format!("{}s ago", s)
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
                .map(|s| fmt_state(&s.active).len())
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
                    fmt_state(&session.active),
                );
            }
        }
        Command::TerminateSession { ref ids }
        | Command::KillSession { ref ids }
        | Command::LockSession { ref ids }
        | Command::UnlockSession { ref ids } => {
            let send: fn(&session_core_session::SessionCoreSession) = match &cli.command {
                Command::TerminateSession { .. } => |obj| obj.send_terminate(),
                Command::KillSession { .. } => |obj| obj.send_kill(),
                Command::LockSession { .. } => |obj| obj.send_lock(),
                Command::UnlockSession { .. } => |obj| obj.send_unlock(),
                _ => unreachable!(),
            };

            if ids.is_empty() {
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
                send(&session.object);

                return Ok(());
            }

            let mut unmatched = Vec::new();
            let mut matched = Vec::new();

            for id in ids.iter() {
                if let Some(session) = state.sessions.iter().find(|session| session.id == *id) {
                    matched.push(session.object.clone());
                } else {
                    unmatched.push(id);
                }
            }

            if !unmatched.is_empty() {
                let ids = unmatched
                    .iter()
                    .map(|id| id.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                eprintln!("{} unknown session ids: {ids}", "error:".red());
                return Ok(());
            }

            for obj in matched {
                send(&obj);
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
