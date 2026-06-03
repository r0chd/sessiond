mod moxsession_info_v1 {
    hyprwire::include_protocol!("moxsession_info_v1");
    pub use client::*;
}

use clap::{Parser, Subcommand};
use hyprwire::client;
use moxsession_info_v1::{moxsession_info_manager, moxsession_info_session};

struct Session {
    id: u32,
    info_object: moxsession_info_session::MoxsessionInfoSession,
    uid: u32,
    username: String,
    tty: String,
    seat: Option<String>,
    leader: u32,
    timestamp: u32,
    done: bool,
}

struct State(Vec<Session>);

impl hyprwire::Dispatch<moxsession_info_manager::MoxsessionInfoManager> for State {
    fn event(
        &mut self,
        object: &moxsession_info_manager::MoxsessionInfoManager,
        event: <moxsession_info_manager::MoxsessionInfoManager as hyprwire::Object>::Event<'_>,
    ) {
        match event {
            moxsession_info_manager::Event::Sessions { session_ids } => {
                for session_id in session_ids {
                    let session = Session {
                        id: session_id,
                        info_object: object.send_get_session_info::<State>(session_id).unwrap(),
                        uid: 0,
                        username: String::new(),
                        tty: String::new(),
                        seat: None,
                        leader: 0,
                        timestamp: 0,
                        done: false,
                    };
                    self.0.push(session);
                }
            }
            moxsession_info_manager::Event::SessionAdded { session_id } => {
                let session = Session {
                    id: session_id,
                    info_object: object.send_get_session_info::<State>(session_id).unwrap(),
                    uid: 0,
                    username: String::new(),
                    tty: String::new(),
                    seat: None,
                    leader: 0,
                    timestamp: 0,
                    done: false,
                };
                self.0.push(session);
            }
            moxsession_info_manager::Event::SessionRemoved { session_id } => {
                self.0.retain(|session| session.id != session_id);
            }
        }
    }
}

impl hyprwire::Dispatch<moxsession_info_session::MoxsessionInfoSession> for State {
    fn event(
        &mut self,
        object: &moxsession_info_session::MoxsessionInfoSession,
        event: <moxsession_info_session::MoxsessionInfoSession as hyprwire::Object>::Event<'_>,
    ) {
        let Some(session) = self
            .0
            .iter_mut()
            .find(|session| session.info_object == *object)
        else {
            return;
        };

        match event {
            moxsession_info_session::Event::Uid { uid } => session.uid = uid,
            moxsession_info_session::Event::Username { username } => session.username = username,
            moxsession_info_session::Event::Tty { tty } => session.tty = tty,
            moxsession_info_session::Event::Seat { seat } => session.seat = Some(seat),
            moxsession_info_session::Event::Leader { leader } => session.leader = leader,
            moxsession_info_session::Event::Timestamp { timestamp } => {
                session.timestamp = timestamp
            }
            moxsession_info_session::Event::Done => session.done = true,
            moxsession_info_session::Event::Destroyed => {}
        }
    }
}

#[derive(Subcommand)]
enum Command {
    #[command(about = "list all active sessions")]
    ListSessions,
}

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub(crate) struct Cli {
    #[command(subcommand)]
    command: Command,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let mut state = State(Vec::new());

    let mut socket = client::Client::connect("/run/mox/moxsession.sock")?;
    let event_queue = socket.new_event_queue();

    socket.add_implementation::<moxsession_info_v1::MoxsessionInfoV1Impl>();

    event_queue.wait_for_handshake(&mut state)?;

    let _manager = socket.bind::<moxsession_info_manager::MoxsessionInfoManager, _>(
        &event_queue,
        &mut state,
        1,
    )?;

    event_queue.roundtrip(&mut state)?;

    while state.0.iter().any(|session| !session.done) {
        event_queue.dispatch_events(&mut state, true)?;
    }

    match cli.command {
        Command::ListSessions => {
            let sessions = &state.0;
            if sessions.is_empty() {
                println!("No active sessions.");
                return Ok(());
            }

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

            let id_w = sessions
                .iter()
                .map(|s| s.id.to_string().len())
                .max()
                .unwrap_or(0)
                .max(2);
            let uid_w = sessions
                .iter()
                .map(|s| s.uid.to_string().len())
                .max()
                .unwrap_or(0)
                .max(3);
            let user_w = sessions
                .iter()
                .map(|s| s.username.len())
                .max()
                .unwrap_or(0)
                .max(4);
            let tty_w = sessions
                .iter()
                .map(|s| s.tty.len())
                .max()
                .unwrap_or(0)
                .max(3);
            let seat_w = sessions
                .iter()
                .map(|s| s.seat.as_deref().unwrap_or("-").len())
                .max()
                .unwrap_or(0)
                .max(4);
            let leader_w = sessions
                .iter()
                .map(|s| s.leader.to_string().len())
                .max()
                .unwrap_or(0)
                .max(6);
            let since_w = sessions
                .iter()
                .map(|s| fmt_ts(s.timestamp).len())
                .max()
                .unwrap_or(0)
                .max(5);

            println!(
                "  {:<id_w$}  {:<uid_w$}  {:<user_w$}  {:<tty_w$}  {:<seat_w$}  {:<leader_w$}  {:<since_w$}",
                "ID", "UID", "USER", "TTY", "SEAT", "LEADER", "SINCE",
            );
            println!(
                "  {}",
                "─".repeat(id_w + 2 + uid_w + 2 + user_w + 2 + tty_w + 2 + seat_w + 2 + leader_w + 2 + since_w)
            );
            for session in sessions {
                println!(
                    "  {:<id_w$}  {:<uid_w$}  {:<user_w$}  {:<tty_w$}  {:<seat_w$}  {:<leader_w$}  {:<since_w$}",
                    session.id,
                    session.uid,
                    session.username,
                    session.tty,
                    session.seat.as_deref().unwrap_or("-"),
                    session.leader,
                    fmt_ts(session.timestamp),
                );
            }
        }
    }

    Ok(())
}
