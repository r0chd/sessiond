mod backend;

#[cfg(feature = "consolekit")]
use backend::consolekit;
#[cfg(feature = "logind")]
use backend::logind;
#[cfg(feature = "sessiond")]
use backend::sessiond;
use std::{ffi, ptr};

struct Libsession {
    backend: Box<dyn backend::Backend>,
}

#[repr(C)]
pub struct libsession_context {
    _private: [u8; 0],
}

#[unsafe(no_mangle)]
pub extern "C" fn libsession_new() -> *const libsession_context {
    #[cfg(feature = "sessiond")]
    if let Some(sessiond) = sessiond::Sessiond::try_connect() {
        return Box::into_raw(Box::new(Libsession {
            backend: Box::new(sessiond),
        }))
        .cast::<libsession_context>();
    };
    #[cfg(feature = "logind")]
    if let Some(logind) = logind::Logind::try_connect() {
        return Box::into_raw(Box::new(Libsession {
            backend: Box::new(logind),
        }))
        .cast::<libsession_context>();
    }
    #[cfg(feature = "consolekit")]
    if let Some(consolekit) = consolekit::ConsoleKit::try_connect() {
        return Box::into_raw(Box::new(Libsession {
            backend: Box::new(consolekit),
        }))
        .cast::<libsession_context>();
    }

    ptr::null()
}

#[unsafe(no_mangle)]
pub extern "C" fn libsession_free(ctx: *mut libsession_context) {
    if ctx.is_null() {
        return;
    }

    unsafe { drop(Box::from_raw(ctx.cast::<Libsession>())) }
}

#[unsafe(no_mangle)]
pub extern "C" fn libsession_get_session_id(ctx: *mut libsession_context) -> ffi::c_int {
    if ctx.is_null() {
        return -1;
    }

    1
}
