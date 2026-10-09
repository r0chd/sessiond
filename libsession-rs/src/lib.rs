#[cfg(test)]
mod tests;

use std::os::fd;
use std::os::fd::{AsFd, AsRawFd};
use std::{ffi, ptr};

pub struct Context(ptr::NonNull<libsession_sys::LibsessionContext>);

impl Context {
    #[must_use]
    pub fn connect() -> Option<Self> {
        ptr::NonNull::new(libsession_sys::libsession_new()).map(Self)
    }

    pub fn get_backend(&self) -> libsession_sys::Result<libsession_sys::LibsessionBackend> {
        let mut backend = libsession_sys::LibsessionBackend::Sessiond;

        libsession_sys::Result::<()>::from(unsafe {
            libsession_sys::libsession_get_backend(self.0.as_ptr(), &raw mut backend)
        })?;

        Ok(backend)
    }

    /// Retrieves the session ID for the current process.
    ///
    /// # Errors
    ///
    /// Returns `Error::NotFound` if the backend cannot resolve the session,
    /// or `Error::InternalError` if it returns a null session ID.
    pub fn get_session(&self) -> libsession_sys::Result<Session> {
        let mut id = ptr::null_mut();

        libsession_sys::Result::<()>::from(unsafe {
            libsession_sys::libsession_get_session(self.0.as_ptr(), &raw mut id)
        })?;

        let id = ptr::NonNull::new(id).ok_or(libsession_sys::Error::InternalError)?;
        Ok(Session(id))
    }

    /// Retrieves the session ID for a process.
    ///
    /// # Errors
    ///
    /// Returns `Error::NotFound` if the backend cannot resolve the
    /// session, or `Error::InternalError` if it returns a null ID.
    pub fn get_session_for_pid(&self, pid: i32) -> libsession_sys::Result<Session> {
        let mut id = ptr::null_mut();
        libsession_sys::Result::<()>::from(unsafe {
            libsession_sys::libsession_pid_get_session(self.0.as_ptr(), pid, &raw mut id)
        })?;

        let id = ptr::NonNull::new(id).ok_or(libsession_sys::Error::InternalError)?;
        Ok(Session(id))
    }

    pub fn get_display_for_uid(&self, uid: u32) -> libsession_sys::Result<Session> {
        let mut id = ptr::null_mut();
        libsession_sys::Result::<()>::from(unsafe {
            libsession_sys::libsession_uid_get_display(self.0.as_ptr(), uid, &raw mut id)
        })?;

        let id = ptr::NonNull::new(id).ok_or(libsession_sys::Error::InternalError)?;
        Ok(Session(id))
    }

    pub fn list_sessions(&self) -> libsession_sys::Result<Vec<Session>> {
        let mut sessions = ptr::null_mut();
        let mut count = 0usize;

        libsession_sys::Result::<()>::from(unsafe {
            libsession_sys::libsession_session_list(
                self.0.as_ptr(),
                &raw mut sessions,
                &raw mut count,
            )
        })?;

        if count == 0 {
            return Ok(Vec::new());
        }

        let sessions = ptr::NonNull::new(sessions).ok_or(libsession_sys::Error::InternalError)?;

        let ids = unsafe { Box::from_raw(ptr::slice_from_raw_parts_mut(sessions.as_ptr(), count)) };

        Ok(ids
            .iter()
            .map(|&id| {
                // Every entry was allocated with Box::into_raw and is non-null.
                Session(unsafe { ptr::NonNull::new_unchecked(id) })
            })
            .collect())
    }

    pub fn is_session_local(&self, session: &Session) -> libsession_sys::Result<bool> {
        let mut is_local = false;

        libsession_sys::Result::<()>::from(unsafe {
            libsession_sys::libsession_session_is_local(
                self.0.as_ptr(),
                session.0.as_ptr(),
                &raw mut is_local,
            )
        })?;

        Ok(is_local)
    }

    pub fn get_owner_uid_for_pid(&self, pid: i32) -> libsession_sys::Result<u32> {
        let mut uid = 0u32;

        libsession_sys::Result::<()>::from(unsafe {
            libsession_sys::libsession_pid_get_owner_uid(self.0.as_ptr(), pid, &raw mut uid)
        })?;

        Ok(uid)
    }

    pub fn get_user_state(
        &self,
        uid: u32,
    ) -> libsession_sys::Result<libsession_sys::LibsessionUserState> {
        let mut state = libsession_sys::LibsessionUserState::Offline;

        libsession_sys::Result::<()>::from(unsafe {
            libsession_sys::libsession_uid_get_state(self.0.as_ptr(), uid, &raw mut state)
        })?;

        Ok(state)
    }

    pub fn get_session_uid(&self, session: &Session) -> libsession_sys::Result<u32> {
        let mut uid = 0u32;

        libsession_sys::Result::<()>::from(unsafe {
            libsession_sys::libsession_session_get_uid(
                self.0.as_ptr(),
                session.0.as_ptr(),
                &raw mut uid,
            )
        })?;

        Ok(uid)
    }

    pub fn get_session_state(
        &self,
        session: &Session,
    ) -> libsession_sys::Result<libsession_sys::LibsessionSessionState> {
        let mut state = libsession_sys::LibsessionSessionState::Unkown;

        libsession_sys::Result::<()>::from(unsafe {
            libsession_sys::libsession_session_get_state(
                self.0.as_ptr(),
                session.0.as_ptr(),
                &raw mut state,
            )
        })?;

        Ok(state)
    }

    pub fn is_session_active(&self, session: &Session) -> libsession_sys::Result<bool> {
        let mut is_active = false;

        libsession_sys::Result::<()>::from(unsafe {
            libsession_sys::libsession_session_is_active(
                self.0.as_ptr(),
                session.0.as_ptr(),
                &raw mut is_active,
            )
        })?;

        Ok(is_active)
    }

    /// Sets the logging callback, or disables callbacks with `None`.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying call fails.
    ///
    /// # Safety
    ///
    /// The callback must remain callable until replaced or this context is
    /// dropped. It must not unwind or access this context reentrantly. The
    /// message pointer is valid only during the callback; it must not be
    /// retained or freed.
    pub unsafe fn set_log_fn(
        &self,
        callback: libsession_sys::LibsessionLogFn,
    ) -> libsession_sys::Result<()> {
        libsession_sys::Result::<()>::from(unsafe {
            libsession_sys::libsession_set_log_fn(self.0.as_ptr(), callback)
        })
    }

    /// Sets the logging level. `Off` disables logging.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying call fails.
    pub fn set_log_level(
        &self,
        level: libsession_sys::LibsessionLogLevel,
    ) -> libsession_sys::Result<()> {
        libsession_sys::Result::<()>::from(unsafe {
            libsession_sys::libsession_set_log_level(self.0.as_ptr(), level)
        })
    }

    /// Retrieves the current logging level.
    #[must_use]
    pub fn get_log_level(&self) -> libsession_sys::LibsessionLogLevel {
        unsafe { libsession_sys::libsession_get_log_level(self.0.as_ptr()) }
    }

    /// Creates a monitor for session changes.
    ///
    /// # Errors
    ///
    /// Returns `Error::Unavailable` if the backend cannot create a monitor.
    pub fn get_monitor(&mut self) -> libsession_sys::Result<Monitor> {
        ptr::NonNull::new(libsession_sys::libsession_monitor_new(self.0.as_ptr()))
            .map(Monitor)
            .ok_or(libsession_sys::Error::Unavailable)
    }
}

pub struct Session(ptr::NonNull<libsession_sys::LibsessionSession>);

impl Session {
    pub fn id(&self) -> libsession_sys::Result<String> {
        let mut id = ptr::null_mut();
        unsafe { libsession_sys::libsession_session_get_id(self.0.as_ptr(), &raw mut id) };

        unsafe { ffi::CString::from_raw(id) }
            .into_string()
            .map_err(|_| libsession_sys::Error::InvalidData)
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        unsafe { libsession_sys::libsession_session_free(self.0.as_ptr()) };
    }
}

pub struct Monitor(ptr::NonNull<libsession_sys::libsession_monitor>);

impl Monitor {
    pub fn flush(&mut self) {
        libsession_sys::libsession_monitor_flush(self.0.as_ptr());
    }
}

impl AsRawFd for Monitor {
    fn as_raw_fd(&self) -> fd::RawFd {
        libsession_sys::libsession_monitor_fd(self.0.as_ptr())
    }
}

impl AsFd for Monitor {
    fn as_fd(&self) -> fd::BorrowedFd<'_> {
        unsafe { fd::BorrowedFd::borrow_raw(self.as_raw_fd()) }
    }
}

impl Drop for Monitor {
    fn drop(&mut self) {
        libsession_sys::libsession_monitor_free(self.0.as_ptr());
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        libsession_sys::libsession_free(self.0.as_ptr());
    }
}
