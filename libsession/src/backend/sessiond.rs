mod session_core_v1 {
    hyprwire::include_protocol!("session_core_v1");
    pub use client::*;
}

use hyprwire::client;
use session_core_v1::{session_core_manager, session_core_session};

pub struct Sessiond {
    eq: hyprwire::EventQueue,
    session_manager: Option<session_core_manager::SessionCoreManager>,
}

impl hyprwire::Dispatch<session_core_manager::SessionCoreManager> for Sessiond {
    fn event(
        &mut self,
        object: &session_core_manager::SessionCoreManager,
        event: <session_core_manager::SessionCoreManager as hyprwire::Object>::Event<'_>,
    ) {
        match event {
            session_core_manager::Event::SessionAdded { session_id } => {
                object.send_get_session::<Self>(session_id);
            }
            session_core_manager::Event::UserAdded { uid } => {}
            _ => {}
        }
    }
}

impl hyprwire::Dispatch<session_core_session::SessionCoreSession> for Sessiond {
    fn event(
        &mut self,
        object: &session_core_session::SessionCoreSession,
        event: <session_core_session::SessionCoreSession as hyprwire::Object>::Event<'_>,
    ) {
        match event {}
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
        };

        eq.wait_for_handshake(&mut sessiond);

        // if we fail to bind manager the connectionn is useless, try another backend
        sessiond.session_manager = Some(socket.bind(&eq, &mut sessiond, 2).ok()?);

        Some(sessiond)
    }
}

impl super::Backend for Sessiond {}
