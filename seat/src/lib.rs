use std::sync::atomic;
use std::{ffi, ptr};

#[repr(C)]
pub struct LibseatSeatListener {
    pub enable_seat: Option<unsafe extern "C" fn(*mut Libseat, *mut libc::c_void)>,
    pub disable_seat: Option<unsafe extern "C" fn(*mut Libseat, *mut libc::c_void)>,
}

#[repr(C)]
pub struct Libseat {
    listener: LibseatSeatListener,
    userdata: *mut libc::c_void,
    seat_name: ffi::CString,
}

type LibseatLogFunc =
    Option<unsafe extern "C" fn(libc::c_int, *const libc::c_char, *mut libc::c_void)>;

static LOG_LEVEL: atomic::AtomicI32 = atomic::AtomicI32::new(1);

fn set_errno(errno: libc::c_int) {
    // SAFETY: libc exposes the thread-local errno location for the current
    // platform. Writing an integer errno value matches the C ABI contract.
    unsafe {
        *libc::__errno_location() = errno;
    }
}

unsafe fn seat_from_ptr<'a>(seat: *mut Libseat) -> Option<&'a mut Libseat> {
    if seat.is_null() {
        set_errno(libc::EINVAL);
        None
    } else {
        // SAFETY: callers pass back the opaque pointer returned by
        // libseat_open_seat. Null is handled above.
        Some(unsafe { &mut *seat })
    }
}

#[unsafe(no_mangle)]
/// # Safety
///
/// `listener` must either be null or point to a valid
/// `LibseatSeatListener` for the duration of this call.
pub unsafe extern "C" fn libseat_open_seat(
    listener: *const LibseatSeatListener,
    userdata: *mut libc::c_void,
) -> *mut Libseat {
    if listener.is_null() {
        set_errno(libc::EINVAL);
        return ptr::null_mut();
    }

    // SAFETY: listener was checked for null and is only copied by value.
    let listener = unsafe { ptr::read(listener) };
    if listener.enable_seat.is_none() || listener.disable_seat.is_none() {
        set_errno(libc::EINVAL);
        return ptr::null_mut();
    }

    let _ = userdata;
    set_errno(libc::ENOSYS);
    ptr::null_mut()
}

#[unsafe(no_mangle)]
/// # Safety
///
/// `seat` must be null or a pointer returned by `libseat_open_seat` that has
/// not been closed.
pub unsafe extern "C" fn libseat_disable_seat(seat: *mut Libseat) -> libc::c_int {
    if unsafe { seat_from_ptr(seat) }.is_none() {
        return -1;
    }

    set_errno(libc::ENOSYS);
    -1
}

#[unsafe(no_mangle)]
/// # Safety
///
/// `seat` must be null or a pointer returned by `libseat_open_seat`. A
/// non-null pointer is consumed and must not be used again.
pub unsafe extern "C" fn libseat_close_seat(seat: *mut Libseat) -> libc::c_int {
    if seat.is_null() {
        set_errno(libc::EINVAL);
        return -1;
    }

    // SAFETY: the pointer must have been returned by libseat_open_seat and is
    // consumed exactly once by libseat_close_seat, matching libseat ownership.
    drop(unsafe { Box::from_raw(seat) });
    0
}

#[unsafe(no_mangle)]
/// # Safety
///
/// `seat` must be null or a live libseat pointer. `path` must be a valid C
/// string and `fd` must be writable when non-null.
pub unsafe extern "C" fn libseat_open_device(
    seat: *mut Libseat,
    path: *const libc::c_char,
    fd: *mut libc::c_int,
) -> libc::c_int {
    if unsafe { seat_from_ptr(seat) }.is_none() || path.is_null() || fd.is_null() {
        set_errno(libc::EINVAL);
        return -1;
    }

    set_errno(libc::ENOSYS);
    -1
}

#[unsafe(no_mangle)]
/// # Safety
///
/// `seat` must be null or a pointer returned by `libseat_open_seat` that has
/// not been closed.
pub unsafe extern "C" fn libseat_close_device(
    seat: *mut Libseat,
    device_id: libc::c_int,
) -> libc::c_int {
    if unsafe { seat_from_ptr(seat) }.is_none() || device_id < 0 {
        set_errno(libc::EINVAL);
        return -1;
    }

    set_errno(libc::ENOSYS);
    -1
}

#[unsafe(no_mangle)]
/// # Safety
///
/// `seat` must be null or a pointer returned by `libseat_open_seat` that has
/// not been closed.
pub unsafe extern "C" fn libseat_seat_name(seat: *mut Libseat) -> *const libc::c_char {
    match unsafe { seat_from_ptr(seat) } {
        Some(seat) => seat.seat_name.as_ptr(),
        None => ptr::null(),
    }
}

#[unsafe(no_mangle)]
/// # Safety
///
/// `seat` must be null or a pointer returned by `libseat_open_seat` that has
/// not been closed.
pub unsafe extern "C" fn libseat_switch_session(
    seat: *mut Libseat,
    session: libc::c_int,
) -> libc::c_int {
    if unsafe { seat_from_ptr(seat) }.is_none() || session <= 0 {
        set_errno(libc::EINVAL);
        return -1;
    }

    set_errno(libc::ENOSYS);
    -1
}

#[unsafe(no_mangle)]
/// # Safety
///
/// `seat` must be null or a pointer returned by `libseat_open_seat` that has
/// not been closed.
pub unsafe extern "C" fn libseat_get_fd(seat: *mut Libseat) -> libc::c_int {
    if unsafe { seat_from_ptr(seat) }.is_none() {
        return -1;
    }

    set_errno(libc::ENOSYS);
    -1
}

#[unsafe(no_mangle)]
/// # Safety
///
/// `seat` must be null or a pointer returned by `libseat_open_seat` that has
/// not been closed.
pub unsafe extern "C" fn libseat_dispatch(seat: *mut Libseat, timeout: libc::c_int) -> libc::c_int {
    if unsafe { seat_from_ptr(seat) }.is_none() || timeout < -1 {
        set_errno(libc::EINVAL);
        return -1;
    }

    set_errno(libc::ENOSYS);
    -1
}

#[unsafe(no_mangle)]
pub extern "C" fn libseat_set_log_handler(handler: LibseatLogFunc) {
    let _ = handler;
}

#[unsafe(no_mangle)]
pub extern "C" fn libseat_set_log_level(level: libc::c_int) {
    LOG_LEVEL.store(level, atomic::Ordering::Relaxed);
}
