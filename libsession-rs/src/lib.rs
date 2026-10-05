use std::os::fd;
use std::os::fd::{AsFd, AsRawFd};
use std::{ffi, ptr, result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Failed,
}

type Result<T> = result::Result<T, Error>;

pub struct Session(ptr::NonNull<libsession_sys::libsession_context>);

impl Session {
    #[must_use]
    pub fn connect() -> Option<Self> {
        ptr::NonNull::new(libsession_sys::libsession_new()).map(Self)
    }

    pub fn id(&self) -> Result<String> {
        let mut id = ptr::null_mut();
        result(unsafe { libsession_sys::libsession_get_session(self.0.as_ptr(), &raw mut id) })?;
        unsafe { take_session_id(id) }
    }

    pub fn id_for_pid(&self, pid: u32) -> Result<String> {
        let pid = pid.try_into().map_err(|_| Error::Failed)?;
        let mut id = ptr::null_mut();
        result(unsafe {
            libsession_sys::libsession_pid_get_session(self.0.as_ptr(), pid, &raw mut id)
        })?;
        unsafe { take_session_id(id) }
    }

    pub fn monitor(&mut self) -> Result<Monitor> {
        ptr::NonNull::new(libsession_sys::libsession_monitor_new(self.0.as_ptr()))
            .map(Monitor)
            .ok_or(Error::Failed)
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
    let id = ptr::NonNull::new(id).ok_or(Error::Failed)?;
    let owned = unsafe { ffi::CStr::from_ptr(id.as_ptr()) }
        .to_string_lossy()
        .into_owned();
    unsafe { libsession_sys::libsession_session_id_free(id.as_ptr()) };
    Ok(owned)
}

const fn result(status: libsession_sys::LibsessionStatus) -> Result<()> {
    match status {
        libsession_sys::LibsessionStatus::Ok => Ok(()),
        libsession_sys::LibsessionStatus::Failed => Err(Error::Failed),
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        libsession_sys::libsession_free(self.0.as_ptr());
    }
}
