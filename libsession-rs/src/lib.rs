use std::{ffi, ptr};

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
}

impl Drop for Session {
    fn drop(&mut self) {
        unsafe { libsession_sys::libsession_free(self.0.as_ptr()) };
    }
}
