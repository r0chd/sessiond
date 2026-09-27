use std::os::fd;
use std::os::fd::AsRawFd;
use std::{io, path};

pub(super) struct InotifyMonitor(inotify::Inotify);

impl InotifyMonitor {
    pub(super) fn new(directory: &path::Path) -> Option<Self> {
        let inotify = inotify::Inotify::init().ok()?;
        inotify
            .watches()
            .add(
                directory,
                inotify::WatchMask::MOVED_TO
                    | inotify::WatchMask::CLOSE_WRITE
                    | inotify::WatchMask::DELETE
                    | inotify::WatchMask::DELETE_SELF
                    | inotify::WatchMask::MOVE_SELF
                    | inotify::WatchMask::ONLYDIR,
            )
            .ok()?;
        Some(Self(inotify))
    }
}

impl super::BackendMonitor for InotifyMonitor {
    fn fd(&self) -> fd::RawFd {
        self.0.as_raw_fd()
    }

    fn flush(&mut self) {
        let mut buffer = [0_u8; 4096];
        loop {
            match self.0.read_events(&mut buffer) {
                Ok(_) => {}
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(_) => break,
            }
        }
    }
}
