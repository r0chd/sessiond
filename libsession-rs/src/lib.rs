use std::{ffi, ptr, result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Unsupported,
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
        result(unsafe { libsession_sys::libsession_get_session(self.0.as_ptr(), &mut id) })?;
        unsafe { take_session_id(id) }
    }

    pub fn id_for_pid(&self, pid: u32) -> Result<String> {
        let pid = pid.try_into().map_err(|_| Error::Failed)?;
        let mut id = ptr::null_mut();
        result(unsafe {
            libsession_sys::libsession_pid_get_session(self.0.as_ptr(), pid, &mut id)
        })?;
        unsafe { take_session_id(id) }
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
        libsession_sys::LibsessionStatus::Unsupported => Err(Error::Unsupported),
        libsession_sys::LibsessionStatus::Failed => Err(Error::Failed),
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        libsession_sys::libsession_free(self.0.as_ptr());
    }
}
