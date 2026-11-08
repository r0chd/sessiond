use std::{ffi, result};

#[repr(C)]
pub struct LibsessionContext {
    _private: [u8; 0],
}

#[repr(C)]
pub struct LibsessionSession {
    _private: [u8; 0],
}

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

impl From<LibsessionStatus> for result::Result<(), Error> {
    fn from(value: LibsessionStatus) -> Self {
        match value {
            LibsessionStatus::Ok => Ok(()),
            LibsessionStatus::InvalidArgument => Err(Error::InvalidArgument),
            LibsessionStatus::NotFound => Err(Error::NotFound),
            LibsessionStatus::InternalError => Err(Error::InternalError),
            LibsessionStatus::PermissionDenied => Err(Error::PermissionDenied),
            LibsessionStatus::Unavailable => Err(Error::Unavailable),
            LibsessionStatus::InvalidData => Err(Error::InvalidData),
            LibsessionStatus::IoError => Err(Error::IoError),
            LibsessionStatus::DbusError => Err(Error::DbusError),
        }
    }
}

pub type Result<T> = result::Result<T, Error>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("invalid argument")]
    InvalidArgument,
    #[error("not found")]
    NotFound,
    #[error("internal error")]
    InternalError,
    #[error("permission denied")]
    PermissionDenied,
    #[error("unavailable")]
    Unavailable,
    #[error("invalid data")]
    InvalidData,
    #[error("I/O error")]
    IoError,
    #[error("D-Bus error")]
    DbusError,
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

#[derive(Debug, PartialEq, Eq)]
#[repr(C)]
pub enum LibsessionBackend {
    Sessiond = 0,
    Logind = 1,
    ConsoleKit = 2,
}

#[allow(non_camel_case_types)]
#[repr(C)]
pub struct libsession_monitor {
    _private: [u8; 0],
}

pub type LibsessionLogFn = Option<unsafe extern "C" fn(LibsessionLogLevel, *const ffi::c_char)>;

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

unsafe extern "C" {
    /// Creates a context using the first available backend, or returns NULL if
    /// none is available.
    ///
    /// The context must only be used on the thread that created it. Free it with
    /// `libsession_free`.
    pub fn libsession_new() -> *mut LibsessionContext;

    /// Retrieves the context's active backend.
    ///
    /// # Safety
    ///
    /// `ctx` must be a valid context.
    /// `ret_backend` must point to writable storage. It is written only on success.
    pub fn libsession_get_backend(
        ctx: *mut LibsessionContext,
        ret_backend: *mut LibsessionBackend,
    ) -> LibsessionStatus;

    /// Frees a context.
    ///
    /// # Safety
    ///
    /// `ctx` must be a context returned by `libsession_new` that has not already
    /// been freed.
    pub fn libsession_free(ctx: *mut LibsessionContext);

    /// Retrieves handles for all sessions.
    ///
    /// Free the list with `libsession_session_list_free`.
    ///
    /// # Safety
    ///
    /// `ctx` must be a valid context.
    /// `ret_sessions` and `ret_count` must point to writable storage. They are
    /// written only on success.
    pub fn libsession_session_list(
        ctx: *mut LibsessionContext,
        ret_sessions: *mut *mut *mut LibsessionSession,
        ret_count: *mut usize,
    ) -> LibsessionStatus;

    /// Frees a session list and the handles in it.
    ///
    /// # Safety
    ///
    /// `sessions` and `count` must come from `libsession_session_list`, and the
    /// list must not already be freed.
    pub fn libsession_session_list_free(sessions: *mut *mut LibsessionSession, count: usize);

    /// Retrieves a session handle for a process.
    ///
    /// Free the handle with `libsession_session_free`.
    ///
    /// # Safety
    ///
    /// `ctx` must be a valid context.
    /// `ret_session` must point to writable storage. It is written only on success.
    pub fn libsession_pid_get_session(
        ctx: *mut LibsessionContext,
        pid: libc::pid_t,
        ret_session: *mut *mut LibsessionSession,
    ) -> LibsessionStatus;

    /// Retrieves a session handle for the current process.
    ///
    /// Free the handle with `libsession_session_free`.
    ///
    /// # Safety
    ///
    /// `ctx` must be a valid context.
    /// `ret_session` must point to writable storage. It is written only on success.
    pub fn libsession_get_session(
        ctx: *mut LibsessionContext,
        ret_session: *mut *mut LibsessionSession,
    ) -> LibsessionStatus;

    /// Frees a session handle.
    ///
    /// # Safety
    ///
    /// `id` must be a handle allocated by this library that has not already been
    /// freed.
    pub fn libsession_session_free(id: *mut LibsessionSession);

    /// Retrieves the UID of the owner of the session containing a process.
    ///
    /// # Safety
    ///
    /// `ctx` must be a valid context.
    /// `ret_uid` must point to writable storage. It is written only on success.
    pub fn libsession_pid_get_owner_uid(
        ctx: *mut LibsessionContext,
        pid: libc::pid_t,
        ret_uid: *mut libc::uid_t,
    ) -> LibsessionStatus;

    /// Retrieves a handle for a user's display session.
    ///
    /// Free the handle with `libsession_session_free`.
    ///
    /// # Safety
    ///
    /// `ctx` must be a valid context.
    /// `ret_session` must point to writable storage. It is written only on success.
    pub fn libsession_uid_get_display(
        ctx: *mut LibsessionContext,
        uid: libc::uid_t,
        ret_session: *mut *mut LibsessionSession,
    ) -> LibsessionStatus;

    /// Retrieves a user's login state.
    ///
    /// # Safety
    ///
    /// `ctx` must be a valid context.
    /// `ret_state` must point to writable storage. It is written only on success.
    pub fn libsession_uid_get_state(
        ctx: *mut LibsessionContext,
        uid: libc::uid_t,
        ret_state: *mut LibsessionUserState,
    ) -> LibsessionStatus;

    /// Retrieves the UID of a session's owner.
    ///
    /// # Safety
    ///
    /// `ctx` must be a valid context.
    /// `session` must be a valid session handle.
    /// `ret_uid` must point to writable storage. It is written only on success.
    pub fn libsession_session_get_uid(
        ctx: *mut LibsessionContext,
        session: *const LibsessionSession,
        ret_uid: *mut libc::uid_t,
    ) -> LibsessionStatus;

    /// Retrieves a session's state.
    ///
    /// # Safety
    ///
    /// `ctx` must be a valid context.
    /// `session` must be a valid session handle.
    /// `ret_state` must point to writable storage. It is written only on success.
    pub fn libsession_session_get_state(
        ctx: *mut LibsessionContext,
        session: *const LibsessionSession,
        ret_state: *mut LibsessionSessionState,
    ) -> LibsessionStatus;

    /// Reports whether a session is active.
    ///
    /// # Safety
    ///
    /// `ctx` must be a valid context.
    /// `session` must be a valid session handle.
    /// `ret_active` must point to writable storage. It is written only on success.
    pub fn libsession_session_is_active(
        ctx: *mut LibsessionContext,
        session: *const LibsessionSession,
        ret_active: *mut bool,
    ) -> LibsessionStatus;

    /// Retrieves a session's virtual terminal number.
    ///
    /// `ret_vt` is set to 0 if the session has no VT.
    ///
    /// # Safety
    ///
    /// `ctx` must be a valid context.
    /// `session` must be a valid session handle.
    /// `ret_vt` must point to writable storage. It is written only on success.
    pub fn libsession_session_get_vt(
        ctx: *mut LibsessionContext,
        session: *const LibsessionSession,
        ret_vt: *mut u32,
    ) -> LibsessionStatus;

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
    pub fn libsession_session_get_tty(
        ctx: *mut LibsessionContext,
        session: *const LibsessionSession,
        ret_tty: *mut *mut ffi::c_char,
    ) -> LibsessionStatus;

    /// Reports whether a session is local.
    ///
    /// # Safety
    ///
    /// `ctx` must be a valid context.
    /// `session` must be a valid session handle.
    /// `ret_local` must point to writable storage. It is written only on success.
    pub fn libsession_session_is_local(
        ctx: *mut LibsessionContext,
        session: *const LibsessionSession,
        ret_local: *mut bool,
    ) -> LibsessionStatus;

    /// Copies a session ID into an allocated, NUL-terminated C string.
    ///
    /// Free the string with `libsession_string_free`.
    ///
    /// # Safety
    ///
    /// `session` must be a valid session handle.
    /// `ret_id` must point to writable storage. It is written only on success.
    pub fn libsession_session_get_id(
        session: *const LibsessionSession,
        ret_id: *mut *mut ffi::c_char,
    ) -> LibsessionStatus;

    /// Frees a string returned by this library.
    ///
    /// # Safety
    ///
    /// `string` must be a string allocated by this library that has not already
    /// been freed.
    pub fn libsession_string_free(string: *mut ffi::c_char);

    /// Creates a monitor for session changes, or returns NULL if the backend does
    /// not support monitoring.
    ///
    /// `ctx` must be a valid context. Free the monitor with
    /// `libsession_monitor_free`.
    pub fn libsession_monitor_new(ctx: *const LibsessionContext) -> *mut libsession_monitor;

    /// Frees a monitor.
    ///
    /// `monitor` must be a monitor allocated by this library that has not already
    /// been freed.
    pub fn libsession_monitor_free(monitor: *mut libsession_monitor);

    /// Discards pending monitor events.
    ///
    /// `monitor` must be a valid monitor.
    pub fn libsession_monitor_flush(monitor: *mut libsession_monitor);

    /// Returns the monitor's pollable file descriptor.
    ///
    /// `monitor` must be a valid monitor.
    pub fn libsession_monitor_fd(monitor: *mut libsession_monitor) -> ffi::c_int;

    /// Sets the context's logging callback, or disables callbacks with `None`.
    ///
    /// # Safety
    ///
    /// `ctx` must be a valid context.
    /// The callback must remain callable until replaced or the context is freed.
    /// It must not unwind, access the context reentrantly, or retain the message
    /// pointer, which is valid only for the duration of the callback.
    pub fn libsession_set_log_fn(
        ctx: *mut LibsessionContext,
        callback: LibsessionLogFn,
    ) -> LibsessionStatus;

    /// Sets the context's logging level.
    ///
    /// # Safety
    ///
    /// `ctx` must be a valid context.
    pub fn libsession_set_log_level(
        ctx: *mut LibsessionContext,
        log_level: LibsessionLogLevel,
    ) -> LibsessionStatus;

    /// Retrieves the context's logging level.
    ///
    /// # Safety
    ///
    /// `ctx` must be a valid context.
    pub fn libsession_get_log_level(ctx: *const LibsessionContext) -> LibsessionLogLevel;
}
