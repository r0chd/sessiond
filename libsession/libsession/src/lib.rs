mod backend;
mod logging;
#[cfg(feature = "sessiond")]
mod sessiond_state;
mod status;

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

#[repr(C)]
pub struct LibsessionContext {
    _private: [u8; 0],
}

pub struct LibsessionSession(String);

#[repr(C)]
pub enum LibsessionStatus {
    Ok = 0,
    InvalidArgument = 1,
    NotFound = 2,
    InternalError = 3,
    PermissionDenied = 4,
    Unavailable = 5,
    InvalidData = 6,
    IoError = 7,
    DbusError = 8,
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

/// Creates a context using the first available backend, or returns NULL if
/// none is available.
///
/// The context must only be used on the thread that created it. Free it with
/// `libsession_free`.
#[unsafe(no_mangle)]
#[must_use]
pub extern "C" fn libsession_new() -> *mut LibsessionContext {
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
                    .cast::<LibsessionContext>();
                }
            }
            #[cfg(feature = "logind")]
            "logind" => {
                if let Some(logind) = logind::Logind::try_connect(rc::Rc::clone(&logger)) {
                    return Box::into_raw(Box::new(Libsession {
                        backend: Box::new(logind),
                        logger,
                    }))
                    .cast::<LibsessionContext>();
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
                    .cast::<LibsessionContext>();
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
        .cast::<LibsessionContext>();
    }
    #[cfg(feature = "logind")]
    if let Some(logind) = logind::Logind::try_connect(rc::Rc::clone(&logger)) {
        return Box::into_raw(Box::new(Libsession {
            backend: Box::new(logind),
            logger,
        }))
        .cast::<LibsessionContext>();
    }
    #[cfg(feature = "consolekit")]
    if let Some(consolekit) = consolekit::ConsoleKit::try_connect(rc::Rc::clone(&logger)) {
        return Box::into_raw(Box::new(Libsession {
            backend: Box::new(consolekit),
            logger,
        }))
        .cast::<LibsessionContext>();
    }

    ptr::null_mut()
}

#[derive(Debug, PartialEq, Eq)]
#[repr(C)]
pub enum LibsessionBackend {
    Sessiond = 0,
    Logind = 1,
    ConsoleKit = 2,
}

/// Retrieves the context's active backend.
///
/// # Safety
///
/// `ctx` must be a valid context.
/// `ret_backend` must point to writable storage. It is written only on success.
#[unsafe(no_mangle)]
#[must_use]
pub unsafe extern "C" fn libsession_get_backend(
    ctx: *mut LibsessionContext,
    ret_backend: *mut LibsessionBackend,
) -> LibsessionStatus {
    if ctx.is_null() || ret_backend.is_null() {
        return LibsessionStatus::InvalidArgument;
    }

    let libsession = unsafe { &mut *ctx.cast::<Libsession>() };

    unsafe { ret_backend.write(libsession.backend.get_backend()) };

    LibsessionStatus::Ok
}

/// Frees a context.
///
/// # Safety
///
/// `ctx` must be a context returned by `libsession_new` that has not already
/// been freed.
#[unsafe(no_mangle)]
pub extern "C" fn libsession_free(ctx: *mut LibsessionContext) {
    if ctx.is_null() {
        return;
    }

    unsafe { drop(Box::from_raw(ctx.cast::<Libsession>())) }
}

/// Retrieves handles for all sessions.
///
/// Free the list with `libsession_session_list_free`.
///
/// # Safety
///
/// `ctx` must be a valid context.
/// `ret_sessions` and `ret_count` must point to writable storage. They are
/// written only on success.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn libsession_session_list(
    ctx: *mut LibsessionContext,
    ret_sessions: *mut *mut *mut LibsessionSession,
    ret_count: *mut usize,
) -> LibsessionStatus {
    if ctx.is_null() || ret_sessions.is_null() || ret_count.is_null() {
        return LibsessionStatus::InvalidArgument;
    }

    let libsession = unsafe { &mut *ctx.cast::<Libsession>() };

    match libsession.backend.list_sessions() {
        Ok(sessions) => {
            let ids = sessions
                .into_iter()
                .map(|session_id| Box::into_raw(Box::new(LibsessionSession(session_id))))
                .collect::<Vec<_>>()
                .into_boxed_slice();

            let count = ids.len();
            let ids = if count == 0 {
                ptr::null_mut()
            } else {
                Box::into_raw(ids).cast::<*mut LibsessionSession>()
            };

            unsafe {
                ret_sessions.write(ids);
                ret_count.write(count);
            }

            LibsessionStatus::Ok
        }
        Err(e) => {
            libsession.logger.log(LibsessionLogLevel::Error, || {
                format!("failed to list sessions: {e}")
            });
            LibsessionStatus::from(&e)
        }
    }
}

/// Frees a session list and the handles in it.
///
/// # Safety
///
/// `sessions` and `count` must come from `libsession_session_list`, and the
/// list must not already be freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn libsession_session_list_free(
    sessions: *mut *mut LibsessionSession,
    count: usize,
) {
    if sessions.is_null() {
        return;
    }

    let slice = ptr::slice_from_raw_parts_mut(sessions, count);
    let ids = unsafe { Box::from_raw(slice) };

    for &id in &ids {
        unsafe { libsession_session_free(id) };
    }
}

fn resolve_pid(pid: libc::pid_t) -> Option<u32> {
    match pid {
        // sd-login treats pid 0 as the calling process
        0 => Some(process::id()),
        pid => pid.try_into().ok(),
    }
}

/// Retrieves a session handle for a process.
///
/// Free the handle with `libsession_session_free`.
///
/// # Safety
///
/// `ctx` must be a valid context.
/// `ret_session` must point to writable storage. It is written only on success.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn libsession_pid_get_session(
    ctx: *mut LibsessionContext,
    pid: libc::pid_t,
    ret_session: *mut *mut LibsessionSession,
) -> LibsessionStatus {
    if ctx.is_null() || ret_session.is_null() {
        return LibsessionStatus::InvalidArgument;
    }

    let libsession = unsafe { &mut *ctx.cast::<Libsession>() };

    let Some(pid) = resolve_pid(pid) else {
        return LibsessionStatus::InvalidArgument;
    };
    let Some(session_id) = libsession.backend.get_session_id(pid) else {
        return LibsessionStatus::NotFound;
    };

    unsafe { ret_session.write(Box::into_raw(Box::new(LibsessionSession(session_id)))) };

    LibsessionStatus::Ok
}

/// Retrieves a session handle for the current process.
///
/// Free the handle with `libsession_session_free`.
///
/// # Safety
///
/// `ctx` must be a valid context.
/// `ret_session` must point to writable storage. It is written only on success.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn libsession_get_session(
    ctx: *mut LibsessionContext,
    ret_session: *mut *mut LibsessionSession,
) -> LibsessionStatus {
    let pid = process::id();
    if let Ok(pid) = pid.try_into() {
        return unsafe { libsession_pid_get_session(ctx, pid, ret_session) };
    }

    LibsessionStatus::InvalidArgument
}

/// Frees a session handle.
///
/// # Safety
///
/// `id` must be a handle allocated by this library that has not already been
/// freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn libsession_session_free(id: *mut LibsessionSession) {
    if !id.is_null() {
        unsafe { drop(Box::from_raw(id)) };
    }
}

/// Retrieves the UID of the owner of the session containing a process.
///
/// # Safety
///
/// `ctx` must be a valid context.
/// `ret_uid` must point to writable storage. It is written only on success.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn libsession_pid_get_owner_uid(
    ctx: *mut LibsessionContext,
    pid: libc::pid_t,
    ret_uid: *mut libc::uid_t,
) -> LibsessionStatus {
    if ctx.is_null() || ret_uid.is_null() {
        return LibsessionStatus::InvalidArgument;
    }
    let Some(resolved_pid) = resolve_pid(pid) else {
        return LibsessionStatus::InvalidArgument;
    };

    let libsession = unsafe { &mut *ctx.cast::<Libsession>() };

    match libsession.backend.get_owner_uid(resolved_pid) {
        Ok(owner_uid) => {
            unsafe { ret_uid.write(owner_uid) };
            LibsessionStatus::Ok
        }
        Err(error) => {
            libsession.logger.log(LibsessionLogLevel::Error, || {
                format!("failed to get owner UID for process {pid}: {error}")
            });
            LibsessionStatus::from(&error)
        }
    }
}

/// Retrieves a handle for a user's display session.
///
/// Free the handle with `libsession_session_free`.
///
/// # Safety
///
/// `ctx` must be a valid context.
/// `ret_session` must point to writable storage. It is written only on success.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn libsession_uid_get_display(
    ctx: *mut LibsessionContext,
    uid: libc::uid_t,
    ret_session: *mut *mut LibsessionSession,
) -> LibsessionStatus {
    if ctx.is_null() || ret_session.is_null() {
        return LibsessionStatus::InvalidArgument;
    }

    let libsession = unsafe { &mut *ctx.cast::<Libsession>() };

    if let Some(session_id) = libsession.backend.get_display(uid) {
        unsafe { ret_session.write(Box::into_raw(Box::new(LibsessionSession(session_id)))) };
        return LibsessionStatus::Ok;
    }

    LibsessionStatus::NotFound
}

/// Retrieves a user's login state.
///
/// # Safety
///
/// `ctx` must be a valid context.
/// `ret_state` must point to writable storage. It is written only on success.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn libsession_uid_get_state(
    ctx: *mut LibsessionContext,
    uid: libc::uid_t,
    ret_state: *mut LibsessionUserState,
) -> LibsessionStatus {
    if ctx.is_null() || ret_state.is_null() {
        return LibsessionStatus::InvalidArgument;
    }

    let libsession = unsafe { &mut *ctx.cast::<Libsession>() };
    let Some(state) = libsession.backend.get_state(uid) else {
        return LibsessionStatus::NotFound;
    };
    unsafe { ret_state.write(state) };

    LibsessionStatus::Ok
}

/// Retrieves the UID of a session's owner.
///
/// # Safety
///
/// `ctx` must be a valid context.
/// `session` must be a valid session handle.
/// `ret_uid` must point to writable storage. It is written only on success.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn libsession_session_get_uid(
    ctx: *mut LibsessionContext,
    session: *const LibsessionSession,
    ret_uid: *mut libc::uid_t,
) -> LibsessionStatus {
    if ctx.is_null() || session.is_null() || ret_uid.is_null() {
        return LibsessionStatus::InvalidArgument;
    }

    let libsession = unsafe { &mut *ctx.cast::<Libsession>() };
    let session = unsafe { &*session.cast::<LibsessionSession>() };

    if let Some(owner_uid) = libsession.backend.get_uid(&session.0) {
        unsafe { ret_uid.write(owner_uid) };
        return LibsessionStatus::Ok;
    }

    LibsessionStatus::NotFound
}

/// Retrieves a session's state.
///
/// # Safety
///
/// `ctx` must be a valid context.
/// `session` must be a valid session handle.
/// `ret_state` must point to writable storage. It is written only on success.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn libsession_session_get_state(
    ctx: *mut LibsessionContext,
    session: *const LibsessionSession,
    ret_state: *mut LibsessionSessionState,
) -> LibsessionStatus {
    if ctx.is_null() || session.is_null() || ret_state.is_null() {
        return LibsessionStatus::InvalidArgument;
    }

    let libsession = unsafe { &mut *ctx.cast::<Libsession>() };
    let session = unsafe { &*session.cast::<LibsessionSession>() };

    let state = match libsession.backend.get_session_state(&session.0) {
        Ok(state) => state,
        Err(error) => return (&error).into(),
    };
    unsafe { ret_state.write(state) };

    LibsessionStatus::Ok
}

/// Reports whether a session is active.
///
/// # Safety
///
/// `ctx` must be a valid context.
/// `session` must be a valid session handle.
/// `ret_active` must point to writable storage. It is written only on success.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn libsession_session_is_active(
    ctx: *mut LibsessionContext,
    session: *const LibsessionSession,
    ret_active: *mut bool,
) -> LibsessionStatus {
    if ctx.is_null() || session.is_null() || ret_active.is_null() {
        return LibsessionStatus::InvalidArgument;
    }

    let libsession = unsafe { &mut *ctx.cast::<Libsession>() };
    let session = unsafe { &*session.cast::<LibsessionSession>() };

    let state = match libsession.backend.get_session_state(&session.0) {
        Ok(state) => state,
        Err(error) => return (&error).into(),
    };
    unsafe { ret_active.write(state == LibsessionSessionState::Active) };

    LibsessionStatus::Ok
}

/// Reports whether a session is local.
///
/// # Safety
///
/// `ctx` must be a valid context.
/// `session` must be a valid session handle.
/// `ret_local` must point to writable storage. It is written only on success.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn libsession_session_is_local(
    ctx: *mut LibsessionContext,
    session: *const LibsessionSession,
    ret_local: *mut bool,
) -> LibsessionStatus {
    if ctx.is_null() || session.is_null() || ret_local.is_null() {
        return LibsessionStatus::InvalidArgument;
    }

    let libsession = unsafe { &mut *ctx.cast::<Libsession>() };
    let session = unsafe { &*session.cast::<LibsessionSession>() };

    let Some(local) = libsession.backend.is_local_session(&session.0) else {
        return LibsessionStatus::NotFound;
    };
    unsafe { ret_local.write(local) };

    LibsessionStatus::Ok
}

/// Retrieves a session's TTY as an allocated, NUL-terminated C string.
///
/// `ret_tty` is set to NULL if the session has no TTY. Free the string with
/// `libsession_string_free`.
///
/// # Safety
///
/// `ctx` must be a valid context.
/// `session` must be a valid session handle.
/// `ret_tty` must point to writable storage. It is written only on success.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn libsession_session_get_tty(
    ctx: *mut LibsessionContext,
    session: *const LibsessionSession,
    ret_tty: *mut *mut ffi::c_char,
) -> LibsessionStatus {
    if ctx.is_null() || session.is_null() || ret_tty.is_null() {
        return LibsessionStatus::InvalidArgument;
    }

    let libsession = unsafe { &mut *ctx.cast::<Libsession>() };
    let session = unsafe { &*session.cast::<LibsessionSession>() };

    match libsession.backend.get_tty(&session.0) {
        Ok(Some(tty)) => {
            let Ok(tty) = ffi::CString::new(tty) else {
                return LibsessionStatus::InvalidData;
            };

            unsafe { ret_tty.write(tty.into_raw()) };
            LibsessionStatus::Ok
        }
        Ok(None) => {
            unsafe { ret_tty.write(ptr::null_mut()) };
            LibsessionStatus::Ok
        }
        Err(error) => LibsessionStatus::from(&error),
    }
}

/// Retrieves a session's virtual terminal number.
///
/// `ret_vt` is set to 0 if the session has no VT.
///
/// # Safety
///
/// `ctx` must be a valid context.
/// `session` must be a valid session handle.
/// `ret_vt` must point to writable storage. It is written only on success.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn libsession_session_get_vt(
    ctx: *mut LibsessionContext,
    session: *const LibsessionSession,
    ret_vt: *mut u32,
) -> LibsessionStatus {
    if ctx.is_null() || session.is_null() || ret_vt.is_null() {
        return LibsessionStatus::InvalidArgument;
    }

    let libsession = unsafe { &mut *ctx.cast::<Libsession>() };
    let session = unsafe { &*session.cast::<LibsessionSession>() };

    match libsession.backend.get_vt(&session.0) {
        Ok(vt) => {
            unsafe { ret_vt.write(vt.unwrap_or(0)) };
            LibsessionStatus::Ok
        }
        Err(error) => LibsessionStatus::from(&error),
    }
}

/// Copies a session ID into an allocated, NUL-terminated C string.
///
/// Free the string with `libsession_string_free`.
///
/// # Safety
///
/// `session` must be a valid session handle.
/// `ret_id` must point to writable storage. It is written only on success.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn libsession_session_get_id(
    session: *const LibsessionSession,
    ret_id: *mut *mut ffi::c_char,
) -> LibsessionStatus {
    if session.is_null() || ret_id.is_null() {
        return LibsessionStatus::InvalidArgument;
    }

    let session = unsafe { &*session.cast::<LibsessionSession>() };

    let Ok(session_id) = ffi::CString::new(session.0.as_str()) else {
        return LibsessionStatus::InvalidData;
    };

    unsafe { ret_id.write(session_id.into_raw()) };

    LibsessionStatus::Ok
}

/// Frees a string returned by this library.
///
/// # Safety
///
/// `string` must be a string allocated by this library that has not already
/// been freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn libsession_string_free(string: *mut ffi::c_char) {
    if !string.is_null() {
        unsafe { drop(ffi::CString::from_raw(string)) };
    }
}

#[allow(non_camel_case_types)]
#[repr(C)]
pub struct libsession_monitor {
    _private: [u8; 0],
}

struct Monitor(Box<dyn backend::BackendMonitor>);

/// Creates a monitor for session changes, or returns NULL if the backend does
/// not support monitoring.
///
/// `ctx` must be a valid context. Free the monitor with
/// `libsession_monitor_free`.
#[unsafe(no_mangle)]
pub extern "C" fn libsession_monitor_new(ctx: *const LibsessionContext) -> *mut libsession_monitor {
    if ctx.is_null() {
        return ptr::null_mut();
    }
    let libsession = unsafe { &*ctx.cast::<Libsession>() };
    if let Some(monitor) = libsession.backend.get_monitor() {
        return Box::into_raw(Box::new(Monitor(monitor))).cast::<libsession_monitor>();
    }

    ptr::null_mut()
}

/// Frees a monitor.
///
/// `monitor` must be a monitor allocated by this library that has not already
/// been freed.
#[unsafe(no_mangle)]
pub extern "C" fn libsession_monitor_free(monitor: *mut libsession_monitor) {
    if monitor.is_null() {
        return;
    }

    unsafe { drop(Box::from_raw(monitor.cast::<Monitor>())) }
}

/// Discards pending monitor events.
///
/// `monitor` must be a valid monitor.
#[unsafe(no_mangle)]
pub extern "C" fn libsession_monitor_flush(monitor: *mut libsession_monitor) {
    if monitor.is_null() {
        return;
    }
    let monitor = unsafe { &mut *monitor.cast::<Monitor>() };
    monitor.0.flush();
}

/// Returns the monitor's pollable file descriptor.
///
/// `monitor` must be a valid monitor.
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
/// `ctx` must be a valid context.
/// The callback must remain callable until replaced or the context is freed.
/// It must not unwind, access the context reentrantly, or retain the message
/// pointer, which is valid only for the duration of the callback.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn libsession_set_log_fn(
    ctx: *mut LibsessionContext,
    callback: LibsessionLogFn,
) -> LibsessionStatus {
    if ctx.is_null() {
        return LibsessionStatus::InvalidArgument;
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
/// `ctx` must be a valid context.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn libsession_set_log_level(
    ctx: *mut LibsessionContext,
    log_level: LibsessionLogLevel,
) -> LibsessionStatus {
    if ctx.is_null() {
        return LibsessionStatus::InvalidArgument;
    }
    let libsession = unsafe { &*ctx.cast::<Libsession>() };

    libsession.logger.set_level(log_level);

    LibsessionStatus::Ok
}

/// Retrieves the context's logging level.
///
/// # Safety
///
/// `ctx` must be a valid context.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn libsession_get_log_level(
    ctx: *const LibsessionContext,
) -> LibsessionLogLevel {
    if ctx.is_null() {
        return LibsessionLogLevel::Off;
    }
    let libsession = unsafe { &*ctx.cast::<Libsession>() };
    libsession.logger.level()
}
