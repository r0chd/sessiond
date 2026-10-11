#[cfg(feature = "consolekit")]
pub mod consolekit;
#[cfg(any(feature = "sessiond", feature = "consolekit"))]
mod inotify;
#[cfg(feature = "logind")]
pub mod logind;
#[cfg(feature = "sessiond")]
pub mod sessiond;

use std::os::fd;
use std::{io, num, result};

pub type Result<T> = result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("backend I/O failed: {0}")]
    Io(#[from] io::Error),

    #[error("invalid owner UID: {0}")]
    ParseUid(#[from] num::ParseIntError),

    #[cfg(feature = "consolekit")]
    #[error("D-Bus operation failed: {0}")]
    Dbus(#[from] zbus::Error),
}

pub trait Backend {
    fn get_backend(&self) -> super::LibsessionBackend;

    fn get_owner_uid(&mut self, pid: u32) -> Result<u32>;

    fn get_display(&mut self, uid: u32) -> Option<String>;

    fn get_state(&mut self, uid: u32) -> Option<crate::LibsessionUserState>;

    fn get_uid(&mut self, session_id: &str) -> Option<u32>;

    fn get_session_id(&mut self, pid: u32) -> Option<String>;

    fn get_session_state(&mut self, session_id: &str) -> Result<crate::LibsessionSessionState>;

    fn is_local_session(&self, session_id: &str) -> Option<bool>;

    fn get_monitor(&self) -> Option<Box<dyn BackendMonitor>>;

    fn list_sessions(&mut self) -> Result<Vec<String>>;

    fn get_vt(&mut self, session_id: &str) -> Result<Option<u32>>;

    fn get_tty(&mut self, session_id: &str) -> Result<Option<String>>;
}

pub trait BackendMonitor {
    fn fd(&self) -> fd::RawFd;

    fn flush(&mut self);
}
