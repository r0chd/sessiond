use anyhow::Context;
use std::os::fd;
use std::os::fd::AsRawFd;
use std::os::unix::fs::FileTypeExt;
use std::os::unix::net;
use std::{fs, io, path};

pub(crate) const DEFAULT_SOCKET_PATH: &str = "/run/seatd.sock";

pub(crate) struct Socket {
    listener: net::UnixListener,
    path: path::PathBuf,
}

impl Socket {
    pub(crate) fn bind(path: &path::Path) -> anyhow::Result<Self> {
        if let Ok(metadata) = fs::symlink_metadata(path) {
            if !metadata.file_type().is_socket() {
                anyhow::bail!("refusing to replace non-socket file at {}", path.display());
            }
            fs::remove_file(path)
                .with_context(|| format!("could not remove stale socket at {}", path.display()))?;
        }

        let listener = net::UnixListener::bind(path)
            .with_context(|| format!("could not bind seatd socket at {}", path.display()))?;
        listener
            .set_nonblocking(true)
            .context("could not set seatd socket nonblocking")?;

        Ok(Self {
            listener,
            path: path.to_path_buf(),
        })
    }

    pub(crate) fn accept_pending(&self) {
        loop {
            match self.listener.accept() {
                Ok((stream, _addr)) => {
                    log::info!("accepted seatd compatibility connection");
                    drop(stream);
                }
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => break,
                Err(err) => {
                    log::error!("could not accept seatd connection: {err}");
                    break;
                }
            }
        }
    }
}

impl AsRawFd for Socket {
    fn as_raw_fd(&self) -> fd::RawFd {
        self.listener.as_raw_fd()
    }
}

impl Drop for Socket {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}
