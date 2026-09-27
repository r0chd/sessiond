use std::os::fd;
use std::os::fd::{AsFd, AsRawFd};
use std::{ffi, io, path};

pub struct Monitor {
    inotify: inotify::Inotify,
    filename: ffi::OsString,
    invalidated: bool,
}

impl Monitor {
    /// # Errors
    /// Returns an error for an invalid file path, a missing parent directory,
    /// or failure to initialize inotify or add the watch.
    pub fn new(path: impl AsRef<path::Path>) -> io::Result<Self> {
        let path = path.as_ref();
        let filename = path.file_name().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "state path needs a filename")
        })?;
        let parent = path.parent().filter(|p| !p.as_os_str().is_empty());
        let inotify = inotify::Inotify::init()?;
        inotify.watches().add(
            parent.unwrap_or_else(|| path::Path::new(".")),
            inotify::WatchMask::CREATE
                | inotify::WatchMask::MODIFY
                | inotify::WatchMask::CLOSE_WRITE
                | inotify::WatchMask::MOVED_TO
                | inotify::WatchMask::MOVED_FROM
                | inotify::WatchMask::DELETE
                | inotify::WatchMask::DELETE_SELF
                | inotify::WatchMask::MOVE_SELF
                | inotify::WatchMask::ONLYDIR,
        )?;
        Ok(Self {
            inotify,
            filename: filename.to_owned(),
            invalidated: false,
        })
    }

    /// # Errors
    /// Returns read errors, or `NotFound` if the watched directory was moved,
    /// deleted, unmounted, or its watch removed. Recreate the monitor in that case.
    pub fn has_changed(&mut self) -> io::Result<bool> {
        if self.invalidated {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "state directory watch invalidated; recreate the monitor",
            ));
        }
        let mut changed = false;
        let mut buffer = [0_u8; 4096];
        loop {
            match self.inotify.read_events(&mut buffer) {
                Ok(events) => {
                    for event in events {
                        if event.mask.intersects(
                            inotify::EventMask::DELETE_SELF
                                | inotify::EventMask::MOVE_SELF
                                | inotify::EventMask::IGNORED
                                | inotify::EventMask::UNMOUNT,
                        ) {
                            self.invalidated = true;
                        }
                        changed |= event.mask.contains(inotify::EventMask::Q_OVERFLOW)
                            || event.name == Some(self.filename.as_os_str());
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                Err(error) => return Err(error),
            }
        }
        if self.invalidated {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "state directory watch invalidated; recreate the monitor",
            ));
        }
        Ok(changed)
    }
}

impl AsRawFd for Monitor {
    fn as_raw_fd(&self) -> fd::RawFd {
        self.inotify.as_raw_fd()
    }
}

impl AsFd for Monitor {
    fn as_fd(&self) -> fd::BorrowedFd<'_> {
        self.inotify.as_fd()
    }
}
