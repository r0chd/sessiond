use super::inotify;
use std::os::unix::ffi::OsStrExt;
use std::{env, ffi, fs, path};

pub struct Sessiond {
    state_path: path::PathBuf,
}

impl Sessiond {
    pub fn try_connect() -> Option<Self> {
        let runtime_dir = env::var_os("SESSIOND_RUNTIME_DIR")
            .map_or_else(|| path::PathBuf::from("/run/sessiond"), path::PathBuf::from);
        let backend = Self {
            state_path: runtime_dir.join("state.toml"),
        };

        sessiond_state::try_read(&backend.state_path).ok()?;

        Some(backend)
    }
}

impl super::Backend for Sessiond {
    fn get_session_id(&mut self, pid: u32) -> Option<String> {
        let cgroup = fs::read_to_string(format!("/proc/{pid}/cgroup")).ok()?;
        cgroup.lines().find_map(|line| {
            let path = line.splitn(3, ':').nth(2)?;
            path.split('/')
                .find_map(|part| part.strip_prefix("session-")?.parse().ok())
        })
    }

    fn get_owner_uid(&mut self, pid: u32) -> Option<u32> {
        let owner_uid = fs::read_to_string(format!("/proc/{pid}/loginuid")).ok()?;
        owner_uid.trim().parse().ok()
    }

    fn get_uid(&mut self, session_id: &ffi::CStr) -> Option<u32> {
        let state = sessiond_state::try_read(&self.state_path).ok()?;
        let session = state
            .sessions
            .get(&session_id.to_string_lossy().to_string())?;

        Some(session.uid)
    }

    fn get_state(&mut self, uid: u32) -> Option<crate::LibsessionUserState> {
        let state = sessiond_state::try_read(&self.state_path).ok()?;
        let user = state.users.get(&uid)?;

        match user.state {
            sessiond_state::UserState::Offline => Some(crate::LibsessionUserState::Offline),
            sessiond_state::UserState::Lingering => Some(crate::LibsessionUserState::Lingering),
            sessiond_state::UserState::Online => Some(crate::LibsessionUserState::Online),
            sessiond_state::UserState::Active => Some(crate::LibsessionUserState::Active),
            sessiond_state::UserState::Closing => Some(crate::LibsessionUserState::Closing),
            sessiond_state::UserState::Unknown => Some(crate::LibsessionUserState::Unknown),
        }
    }

    fn get_display(&mut self, uid: u32) -> Option<String> {
        todo!()
    }

    fn get_session_state(
        &mut self,
        session_id: &ffi::CStr,
    ) -> Option<crate::LibsessionSessionState> {
        let state = sessiond_state::try_read(&self.state_path).ok()?;
        let session = state.sessions.get(session_id.to_str().ok()?)?;

        match session.active {
            sessiond_state::ActiveState::Online => Some(crate::LibsessionSessionState::Online),
            sessiond_state::ActiveState::Closing => Some(crate::LibsessionSessionState::Closing),
            sessiond_state::ActiveState::Opening => Some(crate::LibsessionSessionState::Opening),
            sessiond_state::ActiveState::Active => Some(crate::LibsessionSessionState::Active),
            sessiond_state::ActiveState::Unknown => Some(crate::LibsessionSessionState::Unkown),
        }
    }

    fn is_local_session(&mut self, session_id: &ffi::CStr) -> Option<bool> {
        let state = sessiond_state::try_read(&self.state_path).ok()?;
        let session = state.sessions.get(session_id.to_str().ok()?)?;

        Some(session.remote_host.is_some())
    }

    fn get_monitor(&mut self) -> Option<Box<dyn super::BackendMonitor>> {
        Some(Box::new(inotify::InotifyMonitor::new(
            ffi::CString::new(self.state_path.as_os_str().as_bytes())
                .ok()?
                .as_c_str(),
        )?))
    }
}
