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
        ptr::NonNull::new(unsafe { libsession_sys::libsession_new() }).map(Self)
    }

    #[must_use]
    pub fn id(&self) -> Option<String> {
        let id = ptr::NonNull::new(unsafe {
            libsession_sys::libsession_get_session_id(self.0.as_ptr())
        })?;

        let ptr = unsafe { libsession_sys::libsession_session_id_str(id.as_ptr()) };
        let owned = (!ptr.is_null()).then(|| {
            unsafe { ffi::CStr::from_ptr(ptr) }
                .to_string_lossy()
                .into_owned()
        });

        unsafe { libsession_sys::libsession_session_id_free(id.as_ptr()) };

        owned
    }

    pub fn power_off(&self) -> Result<()> {
        result(unsafe { libsession_sys::libsession_power_off(self.0.as_ptr()) })
    }

    pub fn can_power_off(&self) -> Result<bool> {
        capability(unsafe { libsession_sys::libsession_can_power_off(self.0.as_ptr()) })
    }

    pub fn reboot(&self) -> Result<()> {
        result(unsafe { libsession_sys::libsession_reboot(self.0.as_ptr()) })
    }

    pub fn can_reboot(&self) -> Result<bool> {
        capability(unsafe { libsession_sys::libsession_can_reboot(self.0.as_ptr()) })
    }

    pub fn suspend(&self) -> Result<()> {
        result(unsafe { libsession_sys::libsession_suspend(self.0.as_ptr()) })
    }

    pub fn can_suspend(&self) -> Result<bool> {
        capability(unsafe { libsession_sys::libsession_can_suspend(self.0.as_ptr()) })
    }

    pub fn hibernate(&self) -> Result<()> {
        result(unsafe { libsession_sys::libsession_hibernate(self.0.as_ptr()) })
    }

    pub fn can_hibernate(&self) -> Result<bool> {
        capability(unsafe { libsession_sys::libsession_can_hibernate(self.0.as_ptr()) })
    }

    pub fn hybrid_sleep(&self) -> Result<()> {
        result(unsafe { libsession_sys::libsession_hybrid_sleep(self.0.as_ptr()) })
    }

    pub fn can_hybrid_sleep(&self) -> Result<bool> {
        capability(unsafe { libsession_sys::libsession_can_hybrid_sleep(self.0.as_ptr()) })
    }
}

fn result(status: libsession_sys::LibsessionStatus) -> Result<()> {
    match status {
        libsession_sys::LibsessionStatus::Ok => Ok(()),
        libsession_sys::LibsessionStatus::Unsupported => Err(Error::Unsupported),
        libsession_sys::LibsessionStatus::Failed => Err(Error::Failed),
    }
}

const fn capability(capability: libsession_sys::LibsessionCapability) -> Result<bool> {
    match capability {
        libsession_sys::LibsessionCapability::Unavailable => Ok(false),
        libsession_sys::LibsessionCapability::Available => Ok(true),
        libsession_sys::LibsessionCapability::Unsupported => Err(Error::Unsupported),
        libsession_sys::LibsessionCapability::Failed => Err(Error::Failed),
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        unsafe { libsession_sys::libsession_free(self.0.as_ptr()) };
    }
}
