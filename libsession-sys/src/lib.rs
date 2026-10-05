mod backend;
mod logging;

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
use std::{env, ffi, process, ptr, rc};

struct Libsession {
    backend: Box<dyn backend::Backend>,
    logger: rc::Rc<logging::Logger>,
}

#[allow(non_camel_case_types)]
#[repr(C)]
pub struct libsession_context {
    _private: [u8; 0],
}

#[repr(C)]
pub enum LibsessionStatus {
    Ok = 0,
    Failed = 1,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibsessionUserState {
    Offline = 0,
    Lingering = 1,
    Online = 2,
    Active = 3,
    Closing = 4,
    Unknown = 5,
}

#[repr(C)]
#[derive(PartialEq, Eq)]
pub enum LibsessionSessionState {
    Online = 0,
    Active = 1,
    Closing = 2,
    Opening = 3,
    Unkown = 4,
}

#[unsafe(no_mangle)]
#[must_use]
pub extern "C" fn libsession_new() -> *mut libsession_context {
    let logger = rc::Rc::new(logging::Logger::default());
    if let Ok(backend) = env::var("LIBSESSION_BACKEND") {
        match backend.as_str() {
            #[cfg(feature = "sessiond")]
            "sessiond" => {
                if let Some(sessiond) = sessiond::Sessiond::try_connect(rc::Rc::clone(&logger)) {
                    return Box::into_raw(Box::new(Libsession {
                        backend: Box::new(sessiond),
                        logger,
                    }))
                    .cast::<libsession_context>();
                }
            }
            #[cfg(feature = "logind")]
            "logind" => {
                if let Some(logind) = logind::Logind::try_connect(rc::Rc::clone(&logger)) {
                    return Box::into_raw(Box::new(Libsession {
                        backend: Box::new(logind),
                        logger,
                    }))
                    .cast::<libsession_context>();
                }
            }
            #[cfg(feature = "consolekit")]
            "consolekit" => {
                if let Some(consolekit) =
                    consolekit::ConsoleKit::try_connect(rc::Rc::clone(&logger))
                {
                    return Box::into_raw(Box::new(Libsession {
                        backend: Box::new(consolekit),
                        logger,
                    }))
                    .cast::<libsession_context>();
                }
            }
            _ => {}
        }
    }

    #[cfg(feature = "sessiond")]
    if let Some(sessiond) = sessiond::Sessiond::try_connect(rc::Rc::clone(&logger)) {
        return Box::into_raw(Box::new(Libsession {
            backend: Box::new(sessiond),
            logger,
        }))
        .cast::<libsession_context>();
    }
    #[cfg(feature = "logind")]
    if let Some(logind) = logind::Logind::try_connect(rc::Rc::clone(&logger)) {
        return Box::into_raw(Box::new(Libsession {
            backend: Box::new(logind),
            logger,
        }))
        .cast::<libsession_context>();
    }
    #[cfg(feature = "consolekit")]
    if let Some(consolekit) = consolekit::ConsoleKit::try_connect(rc::Rc::clone(&logger)) {
        return Box::into_raw(Box::new(Libsession {
            backend: Box::new(consolekit),
            logger,
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

/// Retrieves an allocated session ID for a process.
///
/// # Safety
///
/// Non-null `ctx` must be a live context with exclusive access for this call.
/// Non-null `ret_session` must point to writable, aligned pointer storage that
/// does not overlap the context. It is written only on success. Free the
/// resulting string with `libsession_session_id_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn libsession_pid_get_session(
    ctx: *mut libsession_context,
    pid: libc::pid_t,
    ret_session: *mut *mut ffi::c_char,
) -> LibsessionStatus {
    if ctx.is_null() || ret_session.is_null() {
        return LibsessionStatus::Failed;
    }

    let libsession = unsafe { &mut *ctx.cast::<Libsession>() };

    let Ok(pid) = pid.try_into() else {
        return LibsessionStatus::Failed;
    };
    let Some(session_id) = libsession.backend.get_session_id(pid) else {
        return LibsessionStatus::Failed;
    };

    let Ok(session_id) = ffi::CString::new(session_id) else {
        return LibsessionStatus::Failed;
    };

    unsafe { ret_session.write(session_id.into_raw()) };
    LibsessionStatus::Ok
}

/// Retrieves an allocated session ID for the current process.
///
/// # Safety
///
/// The pointer requirements of `libsession_pid_get_session` apply.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn libsession_get_session(
    ctx: *mut libsession_context,
    ret_session: *mut *mut ffi::c_char,
) -> LibsessionStatus {
    let pid = process::id();
    if let Ok(pid) = pid.try_into() {
        return unsafe { libsession_pid_get_session(ctx, pid, ret_session) };
    }

    LibsessionStatus::Failed
}

/// Frees a session ID returned by a session ID or display lookup.
///
/// # Safety
///
/// `id` must be null or a pointer written on success by `libsession_get_session`,
/// `libsession_pid_get_session`, or `libsession_uid_get_display` that has not
/// already been freed. Its
/// terminating NUL must remain at its original position. The pointer must not
/// be used after this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn libsession_session_id_free(id: *mut ffi::c_char) {
    if !id.is_null() {
        unsafe { drop(ffi::CString::from_raw(id)) };
    }
}

#[unsafe(no_mangle)]
/// # Safety
/// `ctx` must be a valid context and `ret_uid` must point to writable storage.
pub unsafe extern "C" fn libsession_pid_get_owner_uid(
    ctx: *mut libsession_context,
    pid: libc::pid_t,
    ret_uid: *mut libc::uid_t,
) -> LibsessionStatus {
    if ctx.is_null() || pid < 0 || ret_uid.is_null() {
        return LibsessionStatus::Failed;
    }

    let libsession = unsafe { &mut *ctx.cast::<Libsession>() };

    if let Some(owner_uid) = libsession.backend.get_owner_uid(pid.cast_unsigned()) {
        unsafe { ret_uid.write(owner_uid) };
        return LibsessionStatus::Ok;
    }

    LibsessionStatus::Failed
}

#[unsafe(no_mangle)]
/// # Safety
/// `ctx` must be valid and `ret_session` must point to writable storage.
pub unsafe extern "C" fn libsession_uid_get_display(
    ctx: *mut libsession_context,
    uid: libc::uid_t,
    ret_session: *mut *mut ffi::c_char,
) -> LibsessionStatus {
    if ctx.is_null() || ret_session.is_null() {
        return LibsessionStatus::Failed;
    }

    let libsession = unsafe { &mut *ctx.cast::<Libsession>() };

    if let Some(session_id) = libsession.backend.get_display(uid)
        && let Ok(session_id) = ffi::CString::new(session_id)
    {
        unsafe { ret_session.write(session_id.into_raw()) };
        return LibsessionStatus::Ok;
    }

    LibsessionStatus::Failed
}

#[unsafe(no_mangle)]
/// # Safety
/// `ctx` must be valid and `ret_state` must point to writable storage.
pub unsafe extern "C" fn libsession_uid_get_state(
    ctx: *mut libsession_context,
    uid: libc::uid_t,
    ret_state: *mut LibsessionUserState,
) -> LibsessionStatus {
    if ctx.is_null() || ret_state.is_null() {
        return LibsessionStatus::Failed;
    }

    let libsession = unsafe { &mut *ctx.cast::<Libsession>() };
    let Some(state) = libsession.backend.get_state(uid) else {
        return LibsessionStatus::Failed;
    };
    unsafe { ret_state.write(state) };

    LibsessionStatus::Ok
}

#[unsafe(no_mangle)]
/// # Safety
/// `ctx` and `session` must be valid, and `ret_uid` must point to writable storage.
pub unsafe extern "C" fn libsession_session_get_uid(
    ctx: *mut libsession_context,
    session: *const ffi::c_char,
    ret_uid: *mut libc::uid_t,
) -> LibsessionStatus {
    if ctx.is_null() || session.is_null() || ret_uid.is_null() {
        return LibsessionStatus::Failed;
    }

    let libsession = unsafe { &mut *ctx.cast::<Libsession>() };
    if let Some(owner_uid) = libsession
        .backend
        .get_uid(unsafe { ffi::CStr::from_ptr(session) })
    {
        unsafe { ret_uid.write(owner_uid) };
        return LibsessionStatus::Ok;
    }

    LibsessionStatus::Failed
}

#[unsafe(no_mangle)]
/// # Safety
/// `ctx` and `session` must be valid, and `ret_state` must point to writable storage.
pub unsafe extern "C" fn libsession_session_get_state(
    ctx: *mut libsession_context,
    session: *const ffi::c_char,
    ret_state: *mut LibsessionSessionState,
) -> LibsessionStatus {
    if ctx.is_null() || session.is_null() || ret_state.is_null() {
        return LibsessionStatus::Failed;
    }

    let libsession = unsafe { &mut *ctx.cast::<Libsession>() };
    let Some(state) = libsession
        .backend
        .get_session_state(unsafe { ffi::CStr::from_ptr(session) })
    else {
        return LibsessionStatus::Failed;
    };
    unsafe { ret_state.write(state) };

    LibsessionStatus::Ok
}

#[unsafe(no_mangle)]
/// # Safety
/// `ctx` and `session` must be valid, and `ret_active` must point to writable storage.
pub unsafe extern "C" fn libsession_session_is_active(
    ctx: *mut libsession_context,
    session: *const ffi::c_char,
    ret_active: *mut bool,
) -> LibsessionStatus {
    if ctx.is_null() || session.is_null() || ret_active.is_null() {
        return LibsessionStatus::Failed;
    }

    let libsession = unsafe { &mut *ctx.cast::<Libsession>() };
    let Some(state) = libsession
        .backend
        .get_session_state(unsafe { ffi::CStr::from_ptr(session) })
    else {
        return LibsessionStatus::Failed;
    };
    unsafe { ret_active.write(state == LibsessionSessionState::Active) };

    LibsessionStatus::Ok
}

/// Reports whether a session is local.
///
/// # Safety
///
/// Non-null `ctx` must be a live context with exclusive access for this call.
/// Non-null `session` must point to a valid NUL-terminated string for this call.
/// Non-null `ret_local` must point to writable, aligned storage that does not
/// overlap the context or session string. It is written only on success.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn libsession_session_is_local(
    ctx: *mut libsession_context,
    session: *const ffi::c_char,
    ret_local: *mut bool,
) -> LibsessionStatus {
    if ctx.is_null() || session.is_null() || ret_local.is_null() {
        return LibsessionStatus::Failed;
    }

    let libsession = unsafe { &mut *ctx.cast::<Libsession>() };
    let Some(local) = libsession
        .backend
        .is_local_session(unsafe { ffi::CStr::from_ptr(session) })
    else {
        return LibsessionStatus::Failed;
    };
    unsafe { ret_local.write(local) };

    LibsessionStatus::Ok
}

#[allow(non_camel_case_types)]
#[repr(C)]
pub struct libsession_monitor {
    _private: [u8; 0],
}

struct Monitor(Box<dyn backend::BackendMonitor>);

#[unsafe(no_mangle)]
pub extern "C" fn libsession_monitor_new(
    ctx: *const libsession_context,
) -> *mut libsession_monitor {
    if ctx.is_null() {
        return ptr::null_mut();
    }
    let libsession = unsafe { &*ctx.cast::<Libsession>() };
    if let Some(monitor) = libsession.backend.get_monitor() {
        return Box::into_raw(Box::new(Monitor(monitor))).cast::<libsession_monitor>();
    }

    ptr::null_mut()
}

#[unsafe(no_mangle)]
pub extern "C" fn libsession_monitor_free(monitor: *mut libsession_monitor) {
    if monitor.is_null() {
        return;
    }

    unsafe { drop(Box::from_raw(monitor.cast::<Monitor>())) }
}

#[unsafe(no_mangle)]
pub extern "C" fn libsession_monitor_flush(monitor: *mut libsession_monitor) {
    if monitor.is_null() {
        return;
    }
    let monitor = unsafe { &mut *monitor.cast::<Monitor>() };
    monitor.0.flush();
}

#[unsafe(no_mangle)]
pub extern "C" fn libsession_monitor_fd(monitor: *mut libsession_monitor) -> ffi::c_int {
    if monitor.is_null() {
        return -1;
    }
    let monitor = unsafe { &*monitor.cast::<Monitor>() };
    monitor.0.fd()
}

pub type LibsessionLogFn = Option<unsafe extern "C" fn(LibsessionLogLevel, *const ffi::c_char)>;

/// Sets the context's logging callback, or disables callbacks with `None`.
///
/// # Safety
///
/// Non-null `ctx` must be a live context on the thread that created it.
/// The callback must remain callable until replaced or the context is freed.
/// It must not unwind, access the context reentrantly, or retain the message
/// pointer, which is valid only for the duration of the callback.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn libsession_set_log_fn(
    ctx: *mut libsession_context,
    callback: LibsessionLogFn,
) -> LibsessionStatus {
    if ctx.is_null() {
        return LibsessionStatus::Failed;
    }
    let libsession = unsafe { &*ctx.cast::<Libsession>() };

    libsession.logger.set_callback(callback);

    LibsessionStatus::Ok
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum LibsessionLogLevel {
    #[default]
    Off = 0,
    Error = 1,
    Warn = 2,
    Info = 3,
    Debug = 4,
    Trace = 5,
}

/// Sets the context's logging level.
///
/// # Safety
///
/// Non-null `ctx` must be a live context on the thread that created it and
/// remain valid for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn libsession_set_log_level(
    ctx: *mut libsession_context,
    log_level: LibsessionLogLevel,
) -> LibsessionStatus {
    if ctx.is_null() {
        return LibsessionStatus::Failed;
    }
    let libsession = unsafe { &*ctx.cast::<Libsession>() };

    libsession.logger.set_level(log_level);

    LibsessionStatus::Ok
}

/// Retrieves the context's logging level.
///
/// # Safety
///
/// Non-null `ctx` must be a live context on the thread that created it and
/// remain valid for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn libsession_get_log_level(
    ctx: *const libsession_context,
) -> LibsessionLogLevel {
    if ctx.is_null() {
        return LibsessionLogLevel::Off;
    }
    let libsession = unsafe { &*ctx.cast::<Libsession>() };
    libsession.logger.level()
}
