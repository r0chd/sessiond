use std::{ffi, ptr, result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Failed,
}

type Result<T> = result::Result<T, Error>;

pub struct Session(ptr::NonNull<crate::libsession_context>);

impl Session {
    #[must_use]
    pub fn connect() -> Option<Self> {
        ptr::NonNull::new(crate::libsession_new()).map(Self)
    }

    pub fn id(&self) -> Result<String> {
        let mut id = ptr::null_mut();
        result(unsafe { crate::libsession_get_session(self.0.as_ptr(), &mut id) })?;
        unsafe { take_session_id(id) }
    }

    pub fn id_for_pid(&self, pid: u32) -> Result<String> {
        let pid = pid.try_into().map_err(|_| Error::Failed)?;
        let mut id = ptr::null_mut();
        result(unsafe { crate::libsession_pid_get_session(self.0.as_ptr(), pid, &mut id) })?;
        unsafe { take_session_id(id) }
    }
}

unsafe fn take_session_id(id: *mut ffi::c_char) -> Result<String> {
    let id = ptr::NonNull::new(id).ok_or(Error::Failed)?;
    let owned = unsafe { ffi::CStr::from_ptr(id.as_ptr()) }
        .to_string_lossy()
        .into_owned();
    unsafe { crate::libsession_session_id_free(id.as_ptr()) };
    Ok(owned)
}

const fn result(status: crate::LibsessionStatus) -> Result<()> {
    match status {
        crate::LibsessionStatus::Ok => Ok(()),
        crate::LibsessionStatus::Failed => Err(Error::Failed),
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        crate::libsession_free(self.0.as_ptr());
    }
}
