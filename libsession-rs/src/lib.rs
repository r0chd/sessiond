use std::os::fd;
use std::os::fd::{AsFd, AsRawFd};
use std::{ffi, ptr, result};

type Result<T> = result::Result<T, libsession_sys::Error>;

pub struct Session(ptr::NonNull<libsession_sys::LibsessionContext>);

impl Session {
    #[must_use]
    pub fn connect() -> Option<Self> {
        ptr::NonNull::new(libsession_sys::libsession_new()).map(Self)
    }

    /// Retrieves the session ID for the current process.
    ///
    /// # Errors
    ///
    /// Returns `Error::NotFound` if the backend cannot resolve the session,
    /// or `Error::InternalError` if it returns a null session ID.
    pub fn id(&self) -> Result<String> {
        let mut id = ptr::null_mut();
        result(unsafe { libsession_sys::libsession_get_session(self.0.as_ptr(), &raw mut id) })?;
        unsafe { take_session_id(id) }
    }

    /// Retrieves the session ID for a process.
    ///
    /// # Errors
    ///
    /// Returns `Error::InvalidArgument` if `pid` is outside the platform's
    /// process ID range, `Error::NotFound` if the backend cannot resolve the
    /// session, or `Error::InternalError` if it returns a null ID.
    pub fn id_for_pid(&self, pid: u32) -> Result<String> {
        let pid = pid
            .try_into()
            .map_err(|_| libsession_sys::Error::InvalidArgument)?;
        let mut id = ptr::null_mut();
        result(unsafe {
            libsession_sys::libsession_pid_get_session(self.0.as_ptr(), pid, &raw mut id)
        })?;
        unsafe { take_session_id(id) }
    }

    /// Creates a monitor for session changes.
    ///
    /// # Errors
    ///
    /// Returns `Error::Unavailable` if the backend cannot create a monitor.
    pub fn monitor(&mut self) -> Result<Monitor> {
        ptr::NonNull::new(libsession_sys::libsession_monitor_new(self.0.as_ptr()))
            .map(Monitor)
            .ok_or(libsession_sys::Error::Unavailable)
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

unsafe fn take_session_id(id: *mut ffi::c_char) -> Result<String> {
    let id = ptr::NonNull::new(id).ok_or(libsession_sys::Error::InternalError)?;
    let owned = unsafe { ffi::CStr::from_ptr(id.as_ptr()) }
        .to_string_lossy()
        .into_owned();
    unsafe { libsession_sys::libsession_session_id_free(id.as_ptr()) };
    Ok(owned)
}

const fn result(status: libsession_sys::LibsessionStatus) -> Result<()> {
    match status {
        libsession_sys::LibsessionStatus::Ok => Ok(()),
        libsession_sys::LibsessionStatus::InvalidArgument => {
            Err(libsession_sys::Error::InvalidArgument)
        }
        libsession_sys::LibsessionStatus::NotFound => Err(libsession_sys::Error::NotFound),
        libsession_sys::LibsessionStatus::InternalError => {
            Err(libsession_sys::Error::InternalError)
        }
        libsession_sys::LibsessionStatus::PermissionDenied => {
            Err(libsession_sys::Error::PermissionDenied)
        }
        libsession_sys::LibsessionStatus::Unavailable => Err(libsession_sys::Error::Unavailable),
        libsession_sys::LibsessionStatus::InvalidData => Err(libsession_sys::Error::InvalidData),
        libsession_sys::LibsessionStatus::IoError => Err(libsession_sys::Error::IoError),
        libsession_sys::LibsessionStatus::DbusError => Err(libsession_sys::Error::DbusError),
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        libsession_sys::libsession_free(self.0.as_ptr());
    }
}
