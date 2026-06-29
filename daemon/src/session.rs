use crate::{session_core_session, session_core_v1};
use std::time;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct SessionId(u32);

impl SessionId {
    pub const fn as_raw(self) -> u32 {
        self.0
    }

    pub const fn from_raw(id: u32) -> Self {
        Self(id)
    }

    pub const fn next(self) -> Self {
        Self(self.0.wrapping_add(1))
    }
}

pub struct Session {
    id: SessionId,
    uid: u32,
    username: String,
    vt: u32,
    seat: Option<String>,
    timestamp: time::SystemTime,
    leader_id: i32,
    objects: Vec<session_core_session::SessionCoreSession>,
    state: session_core_v1::ActiveState,
}

impl Session {
    pub fn new(session_id: SessionId, uid: u32, username: String, vt: u32, leader_id: i32) -> Self {
        Self {
            id: session_id,
            uid,
            username,
            vt,
            seat: None,
            timestamp: time::SystemTime::now(),
            leader_id,
            objects: Vec::new(),
            state: session_core_v1::ActiveState::Opening,
        }
    }

    pub const fn vt(&self) -> u32 {
        self.vt
    }

    pub fn username(&self) -> &str {
        &self.username
    }

    pub const fn uid(&self) -> u32 {
        self.uid
    }

    pub const fn id(&self) -> SessionId {
        self.id
    }

    pub fn objects(&self) -> &[session_core_session::SessionCoreSession] {
        &self.objects
    }

    pub const fn state(&self) -> session_core_v1::ActiveState {
        self.state
    }

    pub fn set_state(&mut self, state: session_core_v1::ActiveState) {
        log::info!(session_id = self.id.as_raw(), vt = self.vt; "session state changed");
        self.state = state;
        for object in &self.objects {
            object.send_active(self.state);
        }
    }

    pub fn set_seat(&mut self, seat: String) {
        if self.seat.as_deref() == Some(seat.as_str()) {
            return;
        }

        log::info!(session_id = self.id.as_raw(), seat = seat.as_str(); "session seat changed");
        self.seat = Some(seat);

        if let Some(seat) = self.seat.as_ref() {
            for object in &self.objects {
                object.send_seat(seat);
            }
        }
    }

    pub fn add_object(&mut self, object: session_core_session::SessionCoreSession) {
        log::debug!(session_id = self.id.as_raw(), uid = object.client().map_or(0, |c| c.creds().uid.as_raw()); "session info requested");
        object.send_session_id(self.id.as_raw());
        object.send_uid(self.uid);
        object.send_username(&self.username);
        object.send_vt(self.vt);
        object.send_active(self.state);
        if let Some(seat) = self.seat.as_ref() {
            object.send_seat(seat);
        }
        object.send_leader(self.leader_id.cast_unsigned());
        let timestamp_secs = self
            .timestamp
            .duration_since(time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let timestamp = u32::try_from(timestamp_secs).unwrap_or(u32::MAX);
        object.send_timestamp(timestamp);
        object.send_done();

        self.objects.push(object);
    }
}

impl hyprwire::Dispatch<session_core_session::SessionCoreSession> for crate::Sessiond {
    fn event(
        &mut self,
        object: &session_core_session::SessionCoreSession,
        event: <session_core_session::SessionCoreSession as hyprwire::Object>::Event<'_>,
    ) {
        let Some(idx) = self.sessions.iter_mut().position(|session| {
            session
                .objects
                .iter()
                .any(|session_object| session_object == object)
        }) else {
            return;
        };

        if matches!(event, session_core_session::Event::Destroy) {
            self.sessions[idx]
                .objects
                .retain(|session_object| session_object != object);
            return;
        }

        let Some(client) = object.client() else {
            return;
        };
        if !client.creds().uid.is_root() && client.creds().uid.as_raw() != self.sessions[idx].uid {
            object.send_error(
                session_core_v1::ErrorCode::PermissionDenied as u32,
                "permission denied",
            );
            return;
        }

        match event {
            session_core_session::Event::Terminate => {
                let session = self.sessions.swap_remove(idx);

                log::info!(session_id = session.id.as_raw(); "terminate requested");
                for obj in &session.objects {
                    obj.send_active(session_core_v1::ActiveState::Closing);
                }
                if let Err(err) = self.cgroup_manager.terminate_session(session.id) {
                    log::error!(session_id = session.id.as_raw(), err = err.to_string().as_str(); "terminate failed");
                }
            }
            session_core_session::Event::Kill => {
                log::info!(session_id = self.sessions[idx].id.as_raw(); "kill requested");
                for obj in &self.sessions[idx].objects {
                    obj.send_active(session_core_v1::ActiveState::Closing);
                }
                if let Err(err) = self.cgroup_manager.kill_session(self.sessions[idx].id) {
                    log::error!(session_id = self.sessions[idx].id.as_raw(), err = err.to_string().as_str(); "kill failed");
                }
                self.sessions.swap_remove(idx);
            }
            session_core_session::Event::Lock => {
                log::info!(session_id = self.sessions[idx].id.as_raw(); "lock requested");
                for object in &self.sessions[idx].objects {
                    object.send_locked();
                    object.send_ok();
                }
            }
            session_core_session::Event::Unlock => {
                log::info!(session_id = self.sessions[idx].id.as_raw(); "unlock requested");
                for object in &self.sessions[idx].objects {
                    object.send_unlocked();
                    object.send_ok();
                }
            }
            _ => unreachable!(),
        }
    }
}
