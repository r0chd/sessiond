mod session_core_v1 {
    hyprwire::include_protocol!("session_core_v1");
    pub use client::*;
}

use hyprwire::client;
use session_core_v1::{session_core_manager, session_core_session};
use std::process;

pub struct Session {
    id: Option<u32>,
    pid: u32,
    object: session_core_session::SessionCoreSession,
}

pub struct Sessiond {
    eq: hyprwire::EventQueue,
    session_manager: Option<session_core_manager::SessionCoreManager>,
    sessions: Vec<Session>,
}

impl hyprwire::Dispatch<session_core_session::SessionCoreSession> for Sessiond {
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
        if let session_core_session::Event::SessionId { session_id } = event {
            session.id = Some(session_id);
        }
    }
}

impl Sessiond {
    pub fn try_connect() -> Option<Self> {
        let Ok(mut socket) = client::Client::connect("/run/sessiond/sessiond.sock") else {
            return None;
        };

        let eq = socket.new_event_queue();

        socket.add_implementation::<session_core_v1::SessionCoreV1>();

        let mut sessiond = Self {
            eq: eq.clone(),
            session_manager: None,
            sessions: Vec::new(),
        };

        eq.wait_for_handshake(&mut sessiond).ok()?;

        // if we fail to bind manager the connectionn is useless, try another backend
        sessiond.session_manager = Some(socket.bind(&eq, &mut sessiond, 3).ok()?);

        Some(sessiond)
    }
}

impl super::Backend for Sessiond {
    fn get_session_id(&mut self) -> Option<String> {
        let pid = process::id();
        let eq = self.eq.clone();

        if let Some(index) = self.sessions.iter().position(|session| session.pid == pid) {
            loop {
                if let Some(id) = self.sessions[index].id {
                    return Some(id.to_string());
                }
                eq.dispatch_events(self, true).ok()?;
            }
        }

        let session_manager = self.session_manager.as_ref()?;
        let session_object = session_manager.send_get_current_session::<Self>()?;

        self.sessions.push(Session {
            id: None,
            pid,
            object: session_object.clone(),
        });

        loop {
            eq.dispatch_events(self, true).ok()?;
            if let Some(session) = self
                .sessions
                .iter()
                .find(|session| session.object == session_object)
                && let Some(id) = session.id
            {
                return Some(id.to_string());
            }
        }
    }
}

hyprwire::delegate_noop!(Sessiond: ignore session_core_manager::SessionCoreManager);
