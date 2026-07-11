mod seat_v1 {
    hyprwire::include_protocol!("seat_v1");
    pub use client::*;
}

use seat_v1::{device, seat, seat_manager};
use std::collections::HashMap;
use std::os::fd::{AsRawFd, OwnedFd};
use std::sync::atomic;
use std::{ffi, ptr};

const SESSIOND_SOCKET: &str = "/run/sessiond.sock";

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
    client: hyprwire::client::Client,
    queue: hyprwire::EventQueue,
    seat: seat::Seat,
    initialized: bool,
    enabled: bool,
    devices: HashMap<libc::c_int, DeviceHandle>,
    pending_open: Option<PendingOpen>,
}

type LibseatLogFunc =
    Option<unsafe extern "C" fn(libc::c_int, *const libc::c_char, *mut libc::c_void)>;

static LOG_LEVEL: atomic::AtomicI32 = atomic::AtomicI32::new(1);

struct DeviceHandle {
    object: device::Device,
    _fd: OwnedFd,
}

enum PendingOpen {
    Waiting {
        _object: device::Device,
    },
    Opened {
        object: device::Device,
        device_id: libc::c_int,
        fd: OwnedFd,
    },
    Failed {
        errno: libc::c_int,
    },
    Revoked,
}

fn set_errno(errno: libc::c_int) {
    // SAFETY: libc exposes the thread-local errno location for the current
    // platform. Writing an integer errno value matches the C ABI contract.
    unsafe {
        *libc::__errno_location() = errno;
    }
}

fn errno_from_u32(errno: u32) -> libc::c_int {
    match libc::c_int::try_from(errno) {
        Ok(errno) if errno > 0 => errno,
        _ => libc::EIO,
    }
}

fn errno_from_io(error: &hyprwire::Error) -> libc::c_int {
    match error {
        hyprwire::Error::Io(error) => error.raw_os_error().unwrap_or(libc::EIO),
        hyprwire::Error::ProtocolNotFound => libc::ENOSYS,
        hyprwire::Error::VersionOutOfRange { .. }
        | hyprwire::Error::VersionNegotiationFailed
        | hyprwire::Error::ProtocolViolation(_)
        | hyprwire::Error::HandshakeTimeout
        | hyprwire::Error::ConnectionClosed => libc::EIO,
    }
}

fn dispatch_once(seat: &mut Libseat, block: bool) -> libc::c_int {
    let queue = seat.queue.clone();
    match queue.dispatch_events(seat, block) {
        Ok(()) => 1,
        Err(hyprwire::Error::ConnectionClosed) if !block => 0,
        Err(err) => {
            set_errno(errno_from_io(&err));
            -1
        }
    }
}

fn dispatch_until<F>(seat: &mut Libseat, mut done: F) -> libc::c_int
where
    F: FnMut(&Libseat) -> bool,
{
    while !done(seat) {
        if dispatch_once(seat, true) < 0 {
            return -1;
        }
    }
    0
}

fn poll_loop_fd(seat: &Libseat, timeout: libc::c_int) -> libc::c_int {
    let mut pollfd = libc::pollfd {
        fd: seat.client.extract_loop_fd().as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };

    // SAFETY: pollfd points to one valid pollfd entry for the duration of the
    // call. The fd is borrowed from the live Hyprwire client.
    let ready = unsafe { libc::poll(&raw mut pollfd, 1, timeout) };
    if ready < 0 {
        return -1;
    }
    if ready == 0 {
        return 0;
    }
    if pollfd.revents & (libc::POLLERR | libc::POLLHUP | libc::POLLNVAL) != 0 {
        set_errno(libc::EIO);
        return -1;
    }
    1
}

impl hyprwire::Dispatch<seat_manager::SeatManager> for Libseat {
    fn event(
        &mut self,
        _object: &seat_manager::SeatManager,
        event: <seat_manager::SeatManager as hyprwire::Object>::Event<'_>,
    ) {
        match event {}
    }
}

impl hyprwire::Dispatch<seat::Seat> for Libseat {
    fn event(&mut self, _object: &seat::Seat, event: <seat::Seat as hyprwire::Object>::Event<'_>) {
        match event {
            seat::Event::Enabled => {
                self.initialized = true;
                self.enabled = true;
                if let Some(enable_seat) = self.listener.enable_seat {
                    let seat = ptr::from_mut::<Self>(self);
                    // SAFETY: listener function pointers are provided by the C
                    // caller at open time and are invoked with the live opaque
                    // seat pointer plus its original userdata.
                    unsafe {
                        enable_seat(seat, self.userdata);
                    }
                }
            }
            seat::Event::Disabled => {
                self.initialized = true;
                self.enabled = false;
                if let Some(disable_seat) = self.listener.disable_seat {
                    let seat = ptr::from_mut::<Self>(self);
                    // SAFETY: see the matching enable callback above.
                    unsafe {
                        disable_seat(seat, self.userdata);
                    }
                }
            }
            seat::Event::SessionSwitched { .. } => {}
            seat::Event::SeatError { .. } => {}
        }
    }
}

impl hyprwire::Dispatch<device::Device> for Libseat {
    fn event(
        &mut self,
        object: &device::Device,
        event: <device::Device as hyprwire::Object>::Event<'_>,
    ) {
        match event {
            device::Event::Opened { device_id, fd, .. } => {
                let Ok(device_id) = libc::c_int::try_from(device_id) else {
                    self.pending_open = Some(PendingOpen::Failed {
                        errno: libc::EOVERFLOW,
                    });
                    return;
                };
                self.pending_open = Some(PendingOpen::Opened {
                    object: object.clone(),
                    device_id,
                    fd,
                });
            }
            device::Event::Failed { errno, .. } => {
                self.pending_open = Some(PendingOpen::Failed {
                    errno: errno_from_u32(errno),
                });
            }
            device::Event::Revoked => {
                if matches!(self.pending_open, Some(PendingOpen::Waiting { .. })) {
                    self.pending_open = Some(PendingOpen::Revoked);
                    return;
                }

                let revoked = self.devices.iter().find_map(|(device_id, handle)| {
                    (handle.object == *object).then_some(*device_id)
                });
                if let Some(device_id) = revoked {
                    self.devices.remove(&device_id);
                }
            }
        }
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

    let mut client = match hyprwire::client::Client::connect(SESSIOND_SOCKET) {
        Ok(client) => client,
        Err(err) => {
            set_errno(errno_from_io(&err));
            return ptr::null_mut();
        }
    };
    client.add_implementation::<seat_v1::SeatV1Impl>();
    let queue = client.new_event_queue();

    // SAFETY: static string has no nul byte
    let seat_name = unsafe { ffi::CString::new("seat0").unwrap_unchecked() };
    let mut state = OpenState;
    if let Err(err) = queue.wait_for_handshake(&mut state) {
        set_errno(errno_from_io(&err));
        return ptr::null_mut();
    }
    let manager = match client.bind::<seat_manager::SeatManager, OpenState>(&queue, &mut state, 1) {
        Ok(manager) => manager,
        Err(err) => {
            set_errno(errno_from_io(&err));
            return ptr::null_mut();
        }
    };
    let Some(seat_object) = manager.send_open_seat::<Libseat>() else {
        set_errno(libc::EIO);
        return ptr::null_mut();
    };

    let mut seat = Box::new(Libseat {
        listener,
        userdata,
        seat_name,
        client,
        queue,
        seat: seat_object,
        initialized: false,
        enabled: false,
        devices: HashMap::new(),
        pending_open: None,
    });

    if dispatch_until(&mut seat, |seat| seat.initialized) < 0 {
        return ptr::null_mut();
    }

    Box::into_raw(seat)
}

struct OpenState;

impl hyprwire::Dispatch<seat_manager::SeatManager> for OpenState {
    fn event(
        &mut self,
        _object: &seat_manager::SeatManager,
        event: <seat_manager::SeatManager as hyprwire::Object>::Event<'_>,
    ) {
        match event {}
    }
}

#[unsafe(no_mangle)]
/// # Safety
///
/// `seat` must be null or a pointer returned by `libseat_open_seat` that has
/// not been closed.
pub unsafe extern "C" fn libseat_disable_seat(seat: *mut Libseat) -> libc::c_int {
    let Some(seat) = (unsafe { seat_from_ptr(seat) }) else {
        return -1;
    };
    seat.seat.send_ack_disabled();
    seat.enabled = false;
    0
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
    let Some(seat) = (unsafe { seat_from_ptr(seat) }) else {
        return -1;
    };
    if path.is_null() || fd.is_null() {
        set_errno(libc::EINVAL);
        return -1;
    }
    if !seat.enabled {
        set_errno(libc::EACCES);
        return -1;
    }

    // SAFETY: path is checked for null above and must be a valid C string per
    // the function contract.
    let Ok(path) = unsafe { ffi::CStr::from_ptr(path) }.to_str() else {
        set_errno(libc::EINVAL);
        return -1;
    };

    let Some(object) = seat.seat.send_open_device::<Libseat>(path) else {
        set_errno(libc::EIO);
        return -1;
    };
    seat.pending_open = Some(PendingOpen::Waiting { _object: object });

    if dispatch_until(seat, |seat| {
        !matches!(seat.pending_open, Some(PendingOpen::Waiting { .. }))
    }) < 0
    {
        seat.pending_open = None;
        return -1;
    }

    match seat.pending_open.take() {
        Some(PendingOpen::Opened {
            object,
            device_id,
            fd: device_fd,
        }) => {
            // SAFETY: fd was checked for null above.
            unsafe {
                *fd = device_fd.as_raw_fd();
            }
            seat.devices.insert(
                device_id,
                DeviceHandle {
                    object,
                    _fd: device_fd,
                },
            );
            device_id
        }
        Some(PendingOpen::Failed { errno }) => {
            set_errno(errno);
            -1
        }
        Some(PendingOpen::Revoked | PendingOpen::Waiting { .. }) | None => {
            set_errno(libc::EIO);
            -1
        }
    }
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
    let Some(seat) = (unsafe { seat_from_ptr(seat) }) else {
        return -1;
    };
    if device_id < 0 {
        set_errno(libc::EINVAL);
        return -1;
    }

    if seat.devices.remove(&device_id).is_some() {
        0
    } else {
        set_errno(libc::ENOENT);
        -1
    }
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
    let Some(seat) = (unsafe { seat_from_ptr(seat) }) else {
        return -1;
    };
    if session <= 0 {
        set_errno(libc::EINVAL);
        return -1;
    }

    let Ok(session) = u32::try_from(session) else {
        set_errno(libc::EINVAL);
        return -1;
    };
    seat.seat.send_switch_session(session);
    0
}

#[unsafe(no_mangle)]
/// # Safety
///
/// `seat` must be null or a pointer returned by `libseat_open_seat` that has
/// not been closed.
pub unsafe extern "C" fn libseat_get_fd(seat: *mut Libseat) -> libc::c_int {
    let Some(seat) = (unsafe { seat_from_ptr(seat) }) else {
        return -1;
    };
    seat.client.extract_loop_fd().as_raw_fd()
}

#[unsafe(no_mangle)]
/// # Safety
///
/// `seat` must be null or a pointer returned by `libseat_open_seat` that has
/// not been closed.
pub unsafe extern "C" fn libseat_dispatch(seat: *mut Libseat, timeout: libc::c_int) -> libc::c_int {
    let Some(seat) = (unsafe { seat_from_ptr(seat) }) else {
        return -1;
    };
    if timeout < -1 {
        set_errno(libc::EINVAL);
        return -1;
    }

    if timeout == 0 {
        let ready = poll_loop_fd(seat, 0);
        if ready <= 0 {
            return ready;
        }
        return dispatch_once(seat, false);
    }

    if timeout > 0 {
        let ready = poll_loop_fd(seat, timeout);
        if ready <= 0 {
            return ready;
        }
        return dispatch_once(seat, false);
    }

    dispatch_once(seat, true)
}

#[unsafe(no_mangle)]
pub extern "C" fn libseat_set_log_handler(handler: LibseatLogFunc) {
    let _ = handler;
}

#[unsafe(no_mangle)]
pub extern "C" fn libseat_set_log_level(level: libc::c_int) {
    LOG_LEVEL.store(level, atomic::Ordering::Relaxed);
}
