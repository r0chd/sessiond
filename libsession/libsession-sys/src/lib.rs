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
    pub fn libsession_new() -> *mut LibsessionContext;

    pub fn libsession_get_backend(
        ctx: *mut LibsessionContext,
        ret_backend: *mut LibsessionBackend,
    ) -> LibsessionStatus;

    pub fn libsession_free(ctx: *mut LibsessionContext);

    pub fn libsession_session_list(
        ctx: *mut LibsessionContext,
        ret_sessions: *mut *mut *mut LibsessionSession,
        ret_count: *mut usize,
    ) -> LibsessionStatus;

    pub fn libsession_session_list_free(sessions: *mut *mut LibsessionSession, count: usize);

    /// Retrieves an allocated session ID for a process.
    ///
    /// # Safety
    ///
    /// Non-null `ctx` must be a live context with exclusive access for this call.
    /// Non-null `ret_session` must point to writable, aligned pointer storage that
    /// does not overlap the context. It is written only on success. Free the
    /// resulting string with `libsession_session_free`.
    pub fn libsession_pid_get_session(
        ctx: *mut LibsessionContext,
        pid: libc::pid_t,
        ret_session: *mut *mut LibsessionSession,
    ) -> LibsessionStatus;

    /// Retrieves an allocated session ID for the current process.
    ///
    /// # Safety
    ///
    /// The pointer requirements of `libsession_pid_get_session` apply.
    pub fn libsession_get_session(
        ctx: *mut LibsessionContext,
        ret_session: *mut *mut LibsessionSession,
    ) -> LibsessionStatus;

    /// Frees an owned session ID handle.
    ///
    /// # Safety
    ///
    /// `id` must be null or a live handle allocated by this library. The caller
    /// must own the handle and ensure it is freed only once, including through
    /// session-list cleanup. No other pointer or reference may access the handle
    /// during this call. The handle and any references into it must not be used
    /// afterward.
    pub fn libsession_session_free(id: *mut LibsessionSession);

    /// # Safety
    /// `ctx` must be a valid context and `ret_uid` must point to writable storage.
    pub fn libsession_pid_get_owner_uid(
        ctx: *mut LibsessionContext,
        pid: libc::pid_t,
        ret_uid: *mut libc::uid_t,
    ) -> LibsessionStatus;

    /// # Safety
    /// `ctx` must be valid and `ret_session` must point to writable storage.
    pub fn libsession_uid_get_display(
        ctx: *mut LibsessionContext,
        uid: libc::uid_t,
        ret_session: *mut *mut LibsessionSession,
    ) -> LibsessionStatus;

    /// # Safety
    /// `ctx` must be valid and `ret_state` must point to writable storage.
    pub fn libsession_uid_get_state(
        ctx: *mut LibsessionContext,
        uid: libc::uid_t,
        ret_state: *mut LibsessionUserState,
    ) -> LibsessionStatus;

    /// # Safety
    /// `ctx` and `session` must be valid, and `ret_uid` must point to writable storage.
    pub fn libsession_session_get_uid(
        ctx: *mut LibsessionContext,
        session: *const LibsessionSession,
        ret_uid: *mut libc::uid_t,
    ) -> LibsessionStatus;

    /// # Safety
    /// `ctx` and `session` must be valid, and `ret_state` must point to writable storage.
    pub fn libsession_session_get_state(
        ctx: *mut LibsessionContext,
        session: *const LibsessionSession,
        ret_state: *mut LibsessionSessionState,
    ) -> LibsessionStatus;

    /// # Safety
    /// `ctx` and `session` must be valid, and `ret_active` must point to writable storage.
    pub fn libsession_session_is_active(
        ctx: *mut LibsessionContext,
        session: *const LibsessionSession,
        ret_active: *mut bool,
    ) -> LibsessionStatus;

    /// Reports whether a session is local.
    ///
    /// # Safety
    ///
    /// Non-null `ctx` must be a live context with exclusive access for this call.
    /// Non-null `session` must point to a valid NUL-terminated string for this call.
    /// Non-null `ret_local` must point to writable, aligned storage that does not
    /// overlap the context or session string. It is written only on success.
    pub fn libsession_session_is_local(
        ctx: *mut LibsessionContext,
        session: *const LibsessionSession,
        ret_local: *mut bool,
    ) -> LibsessionStatus;

    /// Copies a session ID into an allocated, NUL-terminated C string.
    ///
    /// Free the returned string with `libsession_session_id_free`.
    ///
    /// # Safety
    ///
    /// `session` must be null or a live session handle. `ret_id` must point to
    /// writable pointer storage. It is written only on success.
    pub fn libsession_session_get_id(
        session: *const LibsessionSession,
        ret_id: *mut *mut ffi::c_char,
    ) -> LibsessionStatus;

    /// Frees a string returned by this library, such as an ID from
    /// `libsession_session_get_id`.
    ///
    /// # Safety
    ///
    /// `string` must be null or a pointer returned by this library that has not
    /// already been freed. The pointer must not be used after this call.
    pub fn libsession_session_id_free(string: *mut ffi::c_char);

    pub fn libsession_monitor_new(ctx: *const LibsessionContext) -> *mut libsession_monitor;

    pub fn libsession_monitor_free(monitor: *mut libsession_monitor);

    pub fn libsession_monitor_flush(monitor: *mut libsession_monitor);

    pub fn libsession_monitor_fd(monitor: *mut libsession_monitor) -> ffi::c_int;

    /// Sets the context's logging callback, or disables callbacks with `None`.
    ///
    /// # Safety
    ///
    /// Non-null `ctx` must be a live context on the thread that created it.
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
    /// Non-null `ctx` must be a live context on the thread that created it and
    /// remain valid for this call.
    pub fn libsession_set_log_level(
        ctx: *mut LibsessionContext,
        log_level: LibsessionLogLevel,
    ) -> LibsessionStatus;

    /// Retrieves the context's logging level.
    ///
    /// # Safety
    ///
    /// Non-null `ctx` must be a live context on the thread that created it and
    /// remain valid for this call.
    pub fn libsession_get_log_level(ctx: *const LibsessionContext) -> LibsessionLogLevel;
}
