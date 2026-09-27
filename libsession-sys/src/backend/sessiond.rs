use crate::logging;
use std::os::fd::AsRawFd;
use std::{ffi, fs, path, rc};

pub struct Sessiond {
    logger: rc::Rc<logging::Logger>,
    state: sessiond_state::State,
    monitor: sessiond_state::Monitor,
}

impl Sessiond {
    pub fn try_connect(logger: rc::Rc<logging::Logger>) -> Option<Self> {
        let mut state = sessiond_state::State::new();
        state.try_read().ok()?;

        Some(Self {
            logger,
            monitor: state.monitor().ok()?,
            state,
        })
    }
}

impl super::Backend for Sessiond {
    fn get_session_id(&mut self, pid: u32) -> Option<String> {
        self.logger.log(crate::LibsessionLogLevel::Debug, || {
            format!("sessiond: looking up session for process {pid}")
        });
        let cgroup_path = path::PathBuf::from("/proc")
            .join(pid.to_string())
            .join("cgroup");

        let cgroup = fs::read_to_string(cgroup_path).ok()?;
        cgroup.lines().find_map(|line| {
            let path = line.splitn(3, ':').nth(2)?;
            path.split('/')
                .find_map(|part| part.strip_prefix("session-")?.parse().ok())
        })
    }

    fn get_owner_uid(&mut self, pid: u32) -> Option<u32> {
        let path = path::PathBuf::from("/proc")
            .join(pid.to_string())
            .join("loginuid");

        let owner_uid = fs::read_to_string(path).ok()?;
        owner_uid.trim().parse().ok()
    }

    fn get_uid(&mut self, session_id: &ffi::CStr) -> Option<u32> {
        if self.monitor.has_changed().ok()? {
            self.state.try_read().ok()?;
        }

        let session = self
            .state
            .sessions
            .get(&session_id.to_string_lossy().to_string())?;

        Some(session.uid)
    }

    fn get_state(&mut self, uid: u32) -> Option<crate::LibsessionUserState> {
        if self.monitor.has_changed().ok()? {
            self.state.try_read().ok()?;
        }

        let user = self.state.users.get(&uid)?;

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
        if self.monitor.has_changed().ok()? {
            self.state.try_read().ok()?;
        }

        self.state
            .sessions
            .values()
            .find(|session| session.uid == uid)
            .map(|session| session.display.to_string())
    }

    fn get_session_state(
        &mut self,
        session_id: &ffi::CStr,
    ) -> Option<crate::LibsessionSessionState> {
        if self.monitor.has_changed().ok()? {
            self.state.try_read().ok()?;
        }

        let session = self.state.sessions.get(session_id.to_str().ok()?)?;

        match session.active {
            sessiond_state::ActiveState::Online => Some(crate::LibsessionSessionState::Online),
            sessiond_state::ActiveState::Closing => Some(crate::LibsessionSessionState::Closing),
            sessiond_state::ActiveState::Opening => Some(crate::LibsessionSessionState::Opening),
            sessiond_state::ActiveState::Active => Some(crate::LibsessionSessionState::Active),
            sessiond_state::ActiveState::Unknown => Some(crate::LibsessionSessionState::Unkown),
        }
    }

    fn is_local_session(&self, session_id: &ffi::CStr) -> Option<bool> {
        let session = self.state.sessions.get(session_id.to_str().ok()?)?;

        Some(session.remote_host.is_some())
    }

    fn get_monitor(&self) -> Option<Box<dyn super::BackendMonitor>> {
        Some(Box::new(self.state.monitor().ok()?))
    }
}

impl super::BackendMonitor for sessiond_state::Monitor {
    fn fd(&self) -> std::os::fd::RawFd {
        self.as_raw_fd()
    }

    fn flush(&mut self) {
        let _ = self.has_changed();
    }
}
