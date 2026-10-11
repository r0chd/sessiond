use crate::logging;
use crate::sessiond_state;
use crate::sessiond_state::monitor;
use std::os::fd::AsRawFd;
use std::{fs, io, path, rc};

pub struct Sessiond {
    logger: rc::Rc<logging::Logger>,
    state: sessiond_state::State,
    monitor: monitor::Monitor,
}

impl Sessiond {
    pub fn try_connect(logger: rc::Rc<logging::Logger>) -> Option<Self> {
        let mut state = sessiond_state::State::new();
        state.try_read().ok()?;

        Some(Self {
            logger,
            monitor: sessiond_state::State::monitor().ok()?,
            state,
        })
    }
}

impl super::Backend for Sessiond {
    fn get_backend(&self) -> crate::LibsessionBackend {
        crate::LibsessionBackend::Sessiond
    }

    fn get_tty(&mut self, session_id: &str) -> super::Result<Option<String>> {
        self.logger.log(crate::LibsessionLogLevel::Debug, || {
            format!("sessiond: looking up vt for session {session_id}")
        });

        if self.monitor.has_changed()? {
            self.state.try_read()?;
        }

        let session = self
            .state
            .sessions
            .get(session_id)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "session not found"))?;

        Ok(session.tty.as_ref().map(|tty| tty.to_string()))
    }

    fn get_vt(&mut self, session_id: &str) -> super::Result<Option<u32>> {
        self.logger.log(crate::LibsessionLogLevel::Debug, || {
            format!("sessiond: looking up vt for session {session_id}")
        });

        if self.monitor.has_changed()? {
            self.state.try_read()?;
        }

        let session = self
            .state
            .sessions
            .get(session_id)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "session not found"))?;

        Ok(session.vt.map(|vt| vt.get()))
    }

    fn list_sessions(&mut self) -> super::Result<Vec<String>> {
        self.logger.log(crate::LibsessionLogLevel::Debug, || {
            "sessiond: listing all sessions".to_string()
        });

        if self.monitor.has_changed()? {
            self.state.try_read()?;
        }

        Ok(self.state.sessions.keys().cloned().collect())
    }

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

    fn get_owner_uid(&mut self, pid: u32) -> super::Result<u32> {
        if self.monitor.has_changed()? {
            self.state.try_read()?;
        }

        let path = path::PathBuf::from("/proc")
            .join(pid.to_string())
            .join("cgroup");

        let cgroup_path = fs::read_to_string(path)?
            .lines()
            .find_map(|line| line.strip_prefix("0::"))
            .map(|hierarchy_path| {
                path::PathBuf::from("/sys/fs/cgroup").join(hierarchy_path.trim_start_matches('/'))
            })
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::NotFound, "process has no cgroup v2 entry")
            })?;

        let relative_path = cgroup_path
            .strip_prefix(&self.state.cgroup_root)
            .map_err(|_| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    "process is outside sessiond's cgroup root",
                )
            })?;

        let component = relative_path
            .components()
            .next()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "process has no user cgroup"))?;

        let name = component.as_os_str().to_str().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "user cgroup name is not UTF-8")
        })?;

        name.strip_prefix("user-")
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    "process is not in a sessiond user cgroup",
                )
            })?
            .parse::<u32>()
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err).into())
    }

    fn get_uid(&mut self, session_id: &str) -> Option<u32> {
        if self.monitor.has_changed().ok()? {
            self.state.try_read().ok()?;
        }

        let session = self.state.sessions.get(session_id)?;

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
        session_id: &str,
    ) -> super::Result<crate::LibsessionSessionState> {
        if self.monitor.has_changed()? {
            self.state.try_read()?;
        }

        let session = self
            .state
            .sessions
            .get(session_id)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "session not found"))?;

        match session.state {
            sessiond_state::SessionState::Online => Ok(crate::LibsessionSessionState::Online),
            sessiond_state::SessionState::Closing => Ok(crate::LibsessionSessionState::Closing),
            sessiond_state::SessionState::Opening => Ok(crate::LibsessionSessionState::Opening),
            sessiond_state::SessionState::Active => Ok(crate::LibsessionSessionState::Active),
            sessiond_state::SessionState::Unknown => Ok(crate::LibsessionSessionState::Unkown),
        }
    }

    fn is_local_session(&self, session_id: &str) -> Option<bool> {
        let session = self.state.sessions.get(session_id)?;

        Some(session.remote_host.is_none())
    }

    fn get_monitor(&self) -> Option<Box<dyn super::BackendMonitor>> {
        Some(Box::new(sessiond_state::State::monitor().ok()?))
    }
}

impl super::BackendMonitor for monitor::Monitor {
    fn fd(&self) -> std::os::fd::RawFd {
        self.as_raw_fd()
    }

    fn flush(&mut self) {
        let _ = self.has_changed();
    }
}
