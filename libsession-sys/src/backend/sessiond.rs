use hyprwire::client;
use hyprwire_protocols_sessiond::client::session_core::v1::session_core_v1;
use hyprwire_protocols_sessiond::client::session_core::v1::session_core_v1::{
    session_core_manager, session_core_session, session_core_user,
};
use std::ffi;

pub struct Session {
    id: u32,
    pid: u32,
    uid: u32,
    class: session_core_v1::SessionClass,
    state: session_core_v1::ActiveState,
    timestamp: u32,
    done: bool,
    object: session_core_session::SessionCoreSession,
}

impl Session {
    pub const fn new(
        object: session_core_session::SessionCoreSession,
        id: u32,
        pid: u32,
        uid: u32,
    ) -> Self {
        Self {
            id,
            pid,
            uid,
            class: session_core_v1::SessionClass::User,
            state: session_core_v1::ActiveState::Online,
            timestamp: 0,
            done: false,
            object,
        }
    }
}

pub struct User {
    uid: u32,
    state: session_core_v1::UserState,
    object: session_core_user::SessionCoreUser,
    done: bool,
}

pub struct Sessiond {
    eq: hyprwire::EventQueue,
    session_manager: Option<session_core_manager::SessionCoreManager>,
    sessions: Vec<Session>,
    users: Vec<User>,
}

impl hyprwire::Dispatch<session_core_session::SessionCoreSession> for Sessiond {
    fn event(
        &mut self,
        object: &session_core_session::SessionCoreSession,
        event: <session_core_session::SessionCoreSession as hyprwire::Object>::Event<'_>,
    ) {
        let Some(index) = self
            .sessions
            .iter()
            .position(|session| session.object == *object)
        else {
            return;
        };

        match event {
            session_core_session::Event::SessionId { session_id } => {
                self.sessions[index].id = session_id;
            }
            session_core_session::Event::Uid { uid } => {
                self.sessions[index].uid = uid;
            }
            session_core_session::Event::Class { class } => {
                self.sessions[index].class = class;
            }
            session_core_session::Event::Active { state } => {
                self.sessions[index].state = state;
            }
            session_core_session::Event::Timestamp { timestamp } => {
                self.sessions[index].timestamp = timestamp;
            }
            session_core_session::Event::Done => {
                self.sessions[index].done = true;
            }
            session_core_session::Event::Destroyed => {
                self.sessions.remove(index);
            }
            _ => {}
        }
    }
}

impl hyprwire::Dispatch<session_core_user::SessionCoreUser> for Sessiond {
    fn event(
        &mut self,
        object: &session_core_user::SessionCoreUser,
        event: <session_core_user::SessionCoreUser as hyprwire::Object>::Event<'_>,
    ) {
        let Some(index) = self.users.iter().position(|user| user.object == *object) else {
            return;
        };

        match event {
            session_core_user::Event::Uid { uid } => self.users[index].uid = uid,
            session_core_user::Event::State { state } => self.users[index].state = state,
            session_core_user::Event::Done => self.users[index].done = true,
            session_core_user::Event::NotFound => {
                self.users.retain(|candidate| candidate.object != *object);
            }
            _ => {}
        }
    }
}

impl Sessiond {
    fn wait_user(&mut self, object: &session_core_user::SessionCoreUser) -> Option<usize> {
        let eq = self.eq.clone();
        eq.dispatch_events(self, false).ok()?;
        loop {
            let index = self.users.iter().position(|user| user.object == *object)?;
            if self.users[index].done {
                return Some(index);
            }
            eq.dispatch_events(self, true).ok()?;
        }
    }

    fn wait_session(&mut self, object: &session_core_session::SessionCoreSession) -> Option<usize> {
        let eq = self.eq.clone();
        eq.dispatch_events(self, false).ok()?;
        loop {
            let index = self.sessions.iter().position(|s| s.object == *object)?;
            if self.sessions[index].done {
                return Some(index);
            }
            eq.dispatch_events(self, true).ok()?;
        }
    }

    pub fn try_connect() -> Option<Self> {
        let Ok(mut socket) = client::Client::connect("/run/sessiond/sessiond.sock") else {
            return None;
        };

        let eq = socket.new_event_queue();

        socket.add_implementation::<session_core_v1::SessionCoreV1>();

        let mut sessiond = Self {
            eq: eq.clone(),
            sessions: Vec::new(),
            session_manager: None,
            users: Vec::new(),
        };

        eq.wait_for_handshake(&mut sessiond).ok()?;

        // if we fail to bind manager the connectionn is useless, try another backend
        sessiond.session_manager = Some(socket.bind(&eq, &mut sessiond, 3).ok()?);

        eq.roundtrip(&mut sessiond).ok()?;

        Some(sessiond)
    }
}

impl super::Backend for Sessiond {
    fn get_session_id(&mut self, pid: u32) -> Option<String> {
        if let Some(object) = self
            .sessions
            .iter()
            .find(|session| session.pid == pid)
            .map(|session| session.object.clone())
        {
            let index = self.wait_session(&object)?;
            return Some(self.sessions[index].id.to_string());
        }
        let object = self
            .session_manager
            .as_ref()?
            .send_get_session_by_pid::<Self>(pid)?;
        self.sessions.push(Session::new(object.clone(), 0, pid, 0));
        self.wait_session(&object)
            .map(|index| self.sessions[index].id.to_string())
    }

    fn get_owner_uid(&mut self, pid: u32) -> Option<u32> {
        if let Some(object) = self
            .sessions
            .iter()
            .find(|session| session.pid == pid)
            .map(|session| session.object.clone())
        {
            let index = self.wait_session(&object)?;
            return Some(self.sessions[index].uid);
        }
        let object = self
            .session_manager
            .as_ref()?
            .send_get_session_by_pid::<Self>(pid)?;
        self.sessions.push(Session::new(object.clone(), 0, pid, 0));
        self.wait_session(&object)
            .map(|index| self.sessions[index].uid)
    }

    fn get_uid(&mut self, session_id: &ffi::CStr) -> Option<u32> {
        let Ok(session_id) = session_id.to_string_lossy().parse::<u32>() else {
            return None;
        };

        if let Some(object) = self
            .sessions
            .iter()
            .find(|session| session.id == session_id)
            .map(|session| session.object.clone())
        {
            let index = self.wait_session(&object)?;
            return Some(self.sessions[index].uid);
        }
        let object = self
            .session_manager
            .as_ref()?
            .send_get_session::<Self>(session_id)?;
        self.sessions
            .push(Session::new(object.clone(), session_id, 0, 0));
        self.wait_session(&object)
            .map(|index| self.sessions[index].uid)
    }

    fn get_state(&mut self, uid: u32) -> Option<crate::LibsessionUserState> {
        let object = if let Some(object) = self
            .users
            .iter()
            .find(|user| user.uid == uid)
            .map(|user| user.object.clone())
        {
            object
        } else {
            let object = self.session_manager.as_ref()?.send_get_user::<Self>(uid)?;
            self.users.push(User {
                uid,
                state: session_core_v1::UserState::Offline,
                object: object.clone(),
                done: false,
            });
            object
        };
        let index = self.wait_user(&object)?;
        Some(match self.users[index].state {
            session_core_v1::UserState::Offline => crate::LibsessionUserState::Offline,
            session_core_v1::UserState::Lingering => crate::LibsessionUserState::Lingering,
            session_core_v1::UserState::Online => crate::LibsessionUserState::Online,
            session_core_v1::UserState::Active => crate::LibsessionUserState::Active,
            session_core_v1::UserState::Closing => crate::LibsessionUserState::Closing,
            _ => return None,
        })
    }

    fn get_display(&mut self, uid: u32) -> Option<String> {
        // Only the daemon has the complete session list needed to find the display session
        let object = self
            .session_manager
            .as_ref()?
            .send_get_display_session_by_uid::<Self>(uid)?;
        self.sessions.push(Session::new(object.clone(), 0, 0, uid));
        let result = self
            .wait_session(&object)
            .map(|index| self.sessions[index].id.to_string());
        self.sessions.retain(|session| session.object != object);
        drop(object);
        result
    }

    fn get_session_state(
        &mut self,
        session_id: &ffi::CStr,
    ) -> Option<crate::LibsessionSessionState> {
        let Ok(session_id) = session_id.to_string_lossy().parse::<u32>() else {
            return None;
        };

        if let Some(object) = self
            .sessions
            .iter()
            .find(|session| session.id == session_id)
            .map(|session| session.object.clone())
        {
            let index = self.wait_session(&object)?;
            return Some(match self.sessions[index].state {
                session_core_v1::ActiveState::Online => crate::LibsessionSessionState::Online,
                session_core_v1::ActiveState::Active => crate::LibsessionSessionState::Active,
                session_core_v1::ActiveState::Closing => crate::LibsessionSessionState::Closing,
                session_core_v1::ActiveState::Opening => crate::LibsessionSessionState::Opening,
                _ => crate::LibsessionSessionState::Unkown,
            });
        }
        let object = self
            .session_manager
            .as_ref()?
            .send_get_session::<Self>(session_id)?;
        self.sessions
            .push(Session::new(object.clone(), session_id, 0, 0));
        self.wait_session(&object)
            .map(|index| match self.sessions[index].state {
                session_core_v1::ActiveState::Online => crate::LibsessionSessionState::Online,
                session_core_v1::ActiveState::Active => crate::LibsessionSessionState::Active,
                session_core_v1::ActiveState::Closing => crate::LibsessionSessionState::Closing,
                session_core_v1::ActiveState::Opening => crate::LibsessionSessionState::Opening,
                _ => crate::LibsessionSessionState::Unkown,
            })
    }
}

hyprwire::delegate_noop!(Sessiond: ignore session_core_manager::SessionCoreManager);
