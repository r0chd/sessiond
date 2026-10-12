use crate::logging;
use libsystemd_sys::login;
use std::os::fd;
use std::{ffi, io, ptr, rc};

fn sd_string(value: *mut ffi::c_char) -> Option<String> {
    let value = ptr::NonNull::new(value)?;
    let result = unsafe { ffi::CStr::from_ptr(value.as_ptr()) }
        .to_string_lossy()
        .into_owned();
    unsafe { libc::free(value.as_ptr().cast()) };
    Some(result)
}

pub struct Logind {
    logger: rc::Rc<logging::Logger>,
}

impl Logind {
    pub fn try_connect(logger: rc::Rc<logging::Logger>) -> Option<Self> {
        let mut sessions = ptr::null_mut::<*mut ffi::c_char>();
        let result = unsafe { login::sd_get_sessions(&raw mut sessions) };
        if result < 0 {
            return None;
        }
        if !sessions.is_null() {
            for index in 0..usize::try_from(result).ok()? {
                unsafe {
                    libc::free((*sessions.add(index)).cast());
                }
            }
            unsafe { libc::free(sessions.cast()) };
        }
        Some(Self { logger })
    }
}

impl super::Backend for Logind {
    fn get_backend(&self) -> crate::LibsessionBackend {
        crate::LibsessionBackend::Logind
    }

    fn get_tty(&mut self, session_id: &str) -> super::Result<Option<String>> {
        let session_id = ffi::CString::new(session_id)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
        let mut tty = ptr::null_mut();

        let result = unsafe { login::sd_session_get_tty(session_id.as_ptr(), &raw mut tty) };
        if result == -libc::ENODATA {
            return Ok(None);
        }
        if result < 0 {
            return Err(io::Error::from_raw_os_error(-result).into());
        }

        Ok(sd_string(tty).filter(|tty| !tty.is_empty()))
    }

    fn get_vt(&mut self, session_id: &str) -> super::Result<Option<u32>> {
        let session_id = ffi::CString::new(session_id)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
        let mut vt = 0;

        let result = unsafe { login::sd_session_get_vt(session_id.as_ptr(), &raw mut vt) };
        if result == -libc::ENODATA {
            return Ok(None);
        }
        if result < 0 {
            return Err(io::Error::from_raw_os_error(-result).into());
        }

        Ok((vt != 0).then_some(vt))
    }

    fn list_sessions(&mut self) -> super::Result<Vec<String>> {
        self.logger.log(crate::LibsessionLogLevel::Debug, || {
            "logind: listing all sessions".to_string()
        });

        let mut sessions = ptr::null_mut::<*mut ffi::c_char>();
        let result = unsafe { login::sd_get_sessions(&raw mut sessions) };
        let Ok(count) = usize::try_from(result) else {
            return Err(io::Error::from_raw_os_error(-result).into());
        };

        if sessions.is_null() {
            return Ok(Vec::new());
        }

        let mut ids = Vec::with_capacity(count);
        for index in 0..count {
            if let Some(id) = sd_string(unsafe { *sessions.add(index) }) {
                ids.push(id);
            }
        }
        unsafe { libc::free(sessions.cast()) };

        Ok(ids)
    }

    fn is_local_session(&mut self, session_id: &str) -> Option<bool> {
        let session_id = ffi::CString::new(session_id).ok()?;
        let result = unsafe { login::sd_session_is_remote(session_id.as_ptr()) };
        (result >= 0).then_some(result == 0)
    }

    fn get_owner_uid(&mut self, pid: u32) -> super::Result<u32> {
        let pid = libc::pid_t::try_from(pid)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "PID is out of range"))?;
        let mut uid = 0;
        let result = unsafe { login::sd_pid_get_owner_uid(pid, &raw mut uid) };
        if result < 0 {
            return Err(io::Error::from_raw_os_error(-result).into());
        }
        Ok(uid)
    }

    fn get_state(&mut self, uid: u32) -> Option<crate::LibsessionUserState> {
        let mut state = ptr::null_mut();
        let result = unsafe { login::sd_uid_get_state(uid, &raw mut state) };
        let state = (result >= 0).then(|| sd_string(state))??;
        Some(match state.as_str() {
            "offline" => crate::LibsessionUserState::Offline,
            "lingering" => crate::LibsessionUserState::Lingering,
            "online" => crate::LibsessionUserState::Online,
            "active" => crate::LibsessionUserState::Active,
            "closing" => crate::LibsessionUserState::Closing,
            _ => return None,
        })
    }

    fn get_session_id(&mut self, pid: u32) -> Option<String> {
        self.logger.log(crate::LibsessionLogLevel::Debug, || {
            format!("logind: looking up session for process {pid}")
        });
        let mut session = ptr::null_mut();
        let result = unsafe { login::sd_pid_get_session(pid.cast_signed(), &raw mut session) };
        (result >= 0).then(|| sd_string(session))?
    }

    fn get_display(&mut self, uid: u32) -> Option<String> {
        let mut display = ptr::null_mut();
        let result = unsafe { login::sd_uid_get_display(uid, &raw mut display) };
        (result >= 0).then(|| sd_string(display))?
    }

    fn get_uid(&mut self, session_id: &str) -> Option<u32> {
        let session_id = ffi::CString::new(session_id).ok()?;
        let mut uid = 0;
        let result = unsafe { login::sd_session_get_uid(session_id.as_ptr(), &raw mut uid) };
        (result >= 0).then_some(uid)
    }

    fn get_session_state(
        &mut self,
        session_id: &str,
    ) -> super::Result<crate::LibsessionSessionState> {
        let session_id = ffi::CString::new(session_id)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
        let mut state = ptr::null_mut();
        let result = unsafe { login::sd_session_get_state(session_id.as_ptr(), &raw mut state) };

        if result < 0 {
            return Err(io::Error::from_raw_os_error(-result).into());
        }

        let state = sd_string(state).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "logind returned a null session state",
            )
        })?;

        Ok(match state.as_str() {
            "online" => crate::LibsessionSessionState::Online,
            "active" => crate::LibsessionSessionState::Active,
            "closing" => crate::LibsessionSessionState::Closing,
            "opening" => crate::LibsessionSessionState::Opening,
            _ => crate::LibsessionSessionState::Unkown,
        })
    }

    fn get_monitor(&self) -> Option<Box<dyn super::BackendMonitor>> {
        let mut monitor = ptr::null_mut();
        let result = unsafe { login::sd_login_monitor_new(c"session".as_ptr(), &raw mut monitor) };

        if result < 0 {
            return None;
        }

        Some(Box::new(LogindMonitor(monitor)))
    }
}

pub struct LogindMonitor(*mut login::sd_login_monitor);

impl Drop for LogindMonitor {
    fn drop(&mut self) {
        unsafe { login::sd_login_monitor_unref(self.0) };
    }
}

impl super::BackendMonitor for LogindMonitor {
    fn fd(&self) -> fd::RawFd {
        unsafe { login::sd_login_monitor_get_fd(self.0) }
    }

    fn flush(&mut self) {
        if !self.0.is_null() {
            unsafe { login::sd_login_monitor_flush(self.0) };
        }
    }
}
