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

#[unsafe(no_mangle)]
#[must_use]
pub unsafe extern "C" fn libsession_get_backend(
    ctx: *mut LibsessionContext,
    ret_backend: *mut LibsessionBackend,
) -> LibsessionStatus {
    if ctx.is_null() {
        return LibsessionStatus::InvalidArgument;
    }

    let libsession = unsafe { &mut *ctx.cast::<Libsession>() };

    unsafe { ret_backend.write(libsession.backend.get_backend()) };

    LibsessionStatus::Ok
}

#[unsafe(no_mangle)]
pub extern "C" fn libsession_free(ctx: *mut LibsessionContext) {
    if ctx.is_null() {
        return;
    }

    unsafe { drop(Box::from_raw(ctx.cast::<Libsession>())) }
}

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

/// Retrieves an allocated session ID for a process.
///
/// # Safety
///
/// Non-null `ctx` must be a live context with exclusive access for this call.
/// Non-null `ret_session` must point to writable, aligned pointer storage that
/// does not overlap the context. It is written only on success. Free the
/// resulting string with `libsession_session_free`.
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

    let Ok(pid) = pid.try_into() else {
        return LibsessionStatus::InvalidArgument;
    };
    let Some(session_id) = libsession.backend.get_session_id(pid) else {
        return LibsessionStatus::NotFound;
    };

    unsafe { ret_session.write(Box::into_raw(Box::new(LibsessionSession(session_id)))) };

    LibsessionStatus::Ok
}

/// Retrieves an allocated session ID for the current process.
///
/// # Safety
///
/// The pointer requirements of `libsession_pid_get_session` apply.
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

/// Frees an owned session ID handle.
///
/// # Safety
///
/// `id` must be null or a live handle allocated by this library. The caller
/// must own the handle and ensure it is freed only once, including through
/// session-list cleanup. No other pointer or reference may access the handle
/// during this call. The handle and any references into it must not be used
/// afterward.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn libsession_session_free(id: *mut LibsessionSession) {
    if !id.is_null() {
        unsafe { drop(Box::from_raw(id)) };
    }
}

#[unsafe(no_mangle)]
/// # Safety
/// `ctx` must be a valid context and `ret_uid` must point to writable storage.
pub unsafe extern "C" fn libsession_pid_get_owner_uid(
    ctx: *mut LibsessionContext,
    pid: libc::pid_t,
    ret_uid: *mut libc::uid_t,
) -> LibsessionStatus {
    if ctx.is_null() || pid < 0 || ret_uid.is_null() {
        return LibsessionStatus::InvalidArgument;
    }

    let libsession = unsafe { &mut *ctx.cast::<Libsession>() };

    match libsession.backend.get_owner_uid(pid.cast_unsigned()) {
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

#[unsafe(no_mangle)]
/// # Safety
/// `ctx` must be valid and `ret_session` must point to writable storage.
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

#[unsafe(no_mangle)]
/// # Safety
/// `ctx` must be valid and `ret_state` must point to writable storage.
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

#[unsafe(no_mangle)]
/// # Safety
/// `ctx` and `session` must be valid, and `ret_uid` must point to writable storage.
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

#[unsafe(no_mangle)]
/// # Safety
/// `ctx` and `session` must be valid, and `ret_state` must point to writable storage.
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

#[unsafe(no_mangle)]
/// # Safety
/// `ctx` and `session` must be valid, and `ret_active` must point to writable storage.
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
/// Non-null `ctx` must be a live context with exclusive access for this call.
/// Non-null `session` must point to a valid NUL-terminated string for this call.
/// Non-null `ret_local` must point to writable, aligned storage that does not
/// overlap the context or session string. It is written only on success.
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
        Ok(Some(vt)) => unsafe { ret_vt.write(vt) },
        Ok(None) => {}
        Err(error) => return LibsessionStatus::from(&error),
    }

    LibsessionStatus::Ok
}

/// Copies a session ID into an allocated, NUL-terminated C string.
///
/// Free the returned string with `libsession_session_id_free`.
///
/// # Safety
///
/// `session` must be null or a live session handle. `ret_id` must point to
/// writable pointer storage. It is written only on success.
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

/// Frees a string returned by this library, such as an ID from
/// `libsession_session_get_id`.
///
/// # Safety
///
/// `string` must be null or a pointer returned by this library that has not
/// already been freed. The pointer must not be used after this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn libsession_session_id_free(string: *mut ffi::c_char) {
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
/// Non-null `ctx` must be a live context on the thread that created it and
/// remain valid for this call.
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
/// Non-null `ctx` must be a live context on the thread that created it and
/// remain valid for this call.
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
