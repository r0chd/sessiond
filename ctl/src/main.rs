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

            println!(
                "  {:<id_w$}  {:<uid_w$}  {:<user_w$}  {:<tty_w$}",
                "ID", "UID", "USER", "TTY",
            );
            println!(
                "  {}",
                "─".repeat(id_w + 2 + uid_w + 2 + user_w + 2 + tty_w)
            );
            for session in sessions {
                println!(
                    "  {:<id_w$}  {:<uid_w$}  {:<user_w$}  {:<tty_w$}",
                    session.id, session.uid, session.username, session.tty,
                );
            }
        }
    }

    Ok(())
}
