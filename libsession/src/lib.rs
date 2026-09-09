mod backend;

#[cfg(not(any(feature = "sessiond", feature = "logind", feature = "consolekit")))]
compile_error!(
    "libsession requires at least one backend feature: `sessiond`, `logind`, or `consolekit`"
);

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

#[repr(C)]
pub enum LibsessionStatus {
    Ok = 0,
    Unsupported = 1,
    Failed = 2,
}

#[repr(C)]
pub enum LibsessionCapability {
    Unavailable = 0,
    Available = 1,
    Unsupported = 2,
    Failed = 3,
}

fn status(result: Result<(), backend::Error>) -> LibsessionStatus {
    match result {
        Ok(()) => LibsessionStatus::Ok,
        Err(backend::Error::Unsupported) => LibsessionStatus::Unsupported,
        Err(backend::Error::Failed) => LibsessionStatus::Failed,
    }
}

fn capability(result: Result<bool, backend::Error>) -> LibsessionCapability {
    match result {
        Ok(true) => LibsessionCapability::Available,
        Ok(false) => LibsessionCapability::Unavailable,
        Err(backend::Error::Unsupported) => LibsessionCapability::Unsupported,
        Err(backend::Error::Failed) => LibsessionCapability::Failed,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn libsession_new() -> *mut libsession_context {
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

    ptr::null_mut()
}

#[unsafe(no_mangle)]
pub extern "C" fn libsession_free(ctx: *mut libsession_context) {
    if ctx.is_null() {
        return;
    }

    unsafe { drop(Box::from_raw(ctx.cast::<Libsession>())) }
}

#[repr(C)]
pub struct libsession_session_id {
    _private: [u8; 0],
}

#[unsafe(no_mangle)]
pub extern "C" fn libsession_get_session_id(
    ctx: *mut libsession_context,
) -> *mut libsession_session_id {
    if ctx.is_null() {
        return ptr::null_mut();
    }

    let libsession = unsafe { &mut *ctx.cast::<Libsession>() };

    let Some(session_id) = libsession.backend.get_session_id() else {
        return ptr::null_mut();
    };

    let Ok(session_id) = ffi::CString::new(session_id) else {
        return ptr::null_mut();
    };

    Box::into_raw(Box::new(session_id)).cast::<libsession_session_id>()
}

#[unsafe(no_mangle)]
pub extern "C" fn libsession_session_id_str(
    id: *const libsession_session_id,
) -> *const ffi::c_char {
    if id.is_null() {
        return ptr::null();
    }

    unsafe { (*id.cast::<ffi::CString>()).as_ptr() }
}

#[unsafe(no_mangle)]
pub extern "C" fn libsession_session_id_free(id: *mut libsession_session_id) {
    if id.is_null() {
        return;
    }

    unsafe {
        drop(Box::from_raw(id.cast::<ffi::CString>()));
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn libsession_power_off(ctx: *mut libsession_context) -> LibsessionStatus {
    if ctx.is_null() {
        return LibsessionStatus::Failed;
    }

    let libsession = unsafe { &mut *ctx.cast::<Libsession>() };

    status(libsession.backend.power_off())
}

#[unsafe(no_mangle)]
pub extern "C" fn libsession_can_power_off(ctx: *mut libsession_context) -> LibsessionCapability {
    if ctx.is_null() {
        return LibsessionCapability::Failed;
    }

    let libsession = unsafe { &mut *ctx.cast::<Libsession>() };

    capability(libsession.backend.can_power_off())
}

#[unsafe(no_mangle)]
pub extern "C" fn libsession_reboot(ctx: *mut libsession_context) -> LibsessionStatus {
    if ctx.is_null() {
        return LibsessionStatus::Failed;
    }

    let libsession = unsafe { &mut *ctx.cast::<Libsession>() };

    status(libsession.backend.reboot())
}

#[unsafe(no_mangle)]
pub extern "C" fn libsession_can_reboot(ctx: *mut libsession_context) -> LibsessionCapability {
    if ctx.is_null() {
        return LibsessionCapability::Failed;
    }

    let libsession = unsafe { &mut *ctx.cast::<Libsession>() };

    capability(libsession.backend.can_reboot())
}

#[unsafe(no_mangle)]
pub extern "C" fn libsession_suspend(ctx: *mut libsession_context) -> LibsessionStatus {
    if ctx.is_null() {
        return LibsessionStatus::Failed;
    }

    let libsession = unsafe { &mut *ctx.cast::<Libsession>() };

    status(libsession.backend.suspend())
}

#[unsafe(no_mangle)]
pub extern "C" fn libsession_can_suspend(ctx: *mut libsession_context) -> LibsessionCapability {
    if ctx.is_null() {
        return LibsessionCapability::Failed;
    }

    let libsession = unsafe { &mut *ctx.cast::<Libsession>() };

    capability(libsession.backend.can_suspend())
}

#[unsafe(no_mangle)]
pub extern "C" fn libsession_hibernate(ctx: *mut libsession_context) -> LibsessionStatus {
    if ctx.is_null() {
        return LibsessionStatus::Failed;
    }

    let libsession = unsafe { &mut *ctx.cast::<Libsession>() };

    status(libsession.backend.hibernate())
}

#[unsafe(no_mangle)]
pub extern "C" fn libsession_can_hibernate(ctx: *mut libsession_context) -> LibsessionCapability {
    if ctx.is_null() {
        return LibsessionCapability::Failed;
    }

    let libsession = unsafe { &mut *ctx.cast::<Libsession>() };

    capability(libsession.backend.can_hibernate())
}

#[unsafe(no_mangle)]
pub extern "C" fn libsession_hybrid_sleep(ctx: *mut libsession_context) -> LibsessionStatus {
    if ctx.is_null() {
        return LibsessionStatus::Failed;
    }

    let libsession = unsafe { &mut *ctx.cast::<Libsession>() };

    status(libsession.backend.hybrid_sleep())
}

#[unsafe(no_mangle)]
pub extern "C" fn libsession_can_hybrid_sleep(
    ctx: *mut libsession_context,
) -> LibsessionCapability {
    if ctx.is_null() {
        return LibsessionCapability::Failed;
    }

    let libsession = unsafe { &mut *ctx.cast::<Libsession>() };

    capability(libsession.backend.can_hybrid_sleep())
}
