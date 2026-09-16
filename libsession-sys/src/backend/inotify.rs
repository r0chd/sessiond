use std::os::fd;
use std::os::fd::{AsRawFd, FromRawFd};
use std::{ffi, io};

pub(super) struct InotifyMonitor(fd::OwnedFd);

impl InotifyMonitor {
    pub(super) fn new(directory: &ffi::CStr) -> Option<Self> {
        let raw = unsafe { libc::inotify_init1(libc::IN_NONBLOCK | libc::IN_CLOEXEC) };
        if raw < 0 {
            return None;
        }
        let fd = unsafe { fd::OwnedFd::from_raw_fd(raw) };
        let mask = libc::IN_MOVED_TO
            | libc::IN_CLOSE_WRITE
            | libc::IN_DELETE
            | libc::IN_DELETE_SELF
            | libc::IN_MOVE_SELF
            | libc::IN_ONLYDIR;
        let watch = unsafe { libc::inotify_add_watch(fd.as_raw_fd(), directory.as_ptr(), mask) };
        (watch >= 0).then_some(Self(fd))
    }
}

impl super::BackendMonitor for InotifyMonitor {
    fn fd(&self) -> fd::RawFd {
        self.0.as_raw_fd()
    }

    fn flush(&mut self) {
        let mut buffer = [0_u8; 4096];
        loop {
            let read =
                unsafe { libc::read(self.0.as_raw_fd(), buffer.as_mut_ptr().cast(), buffer.len()) };
            if read > 0 {
                continue;
            }
            if read < 0 && io::Error::last_os_error().kind() == io::ErrorKind::Interrupted {
                continue;
            }
            break;
        }
    }
}
