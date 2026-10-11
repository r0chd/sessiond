use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::{fs, io, num, path, result, sync};
use toml::{de, ser};

#[cfg(feature = "monitor")]
pub mod monitor;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SessionClass {
    #[default]
    User,
    Greeter,
    LockScreen,
    Background,
    BackgroundLight,
    Manager,
    ManagerEarly,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SessionState {
    #[default]
    Online,
    Active,
    Closing,
    Opening,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum UserState {
    Offline,
    Lingering,
    Online,
    Active,
    Closing,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct User {
    pub state: UserState,
}

#[derive(Default, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct State {
    pub cgroup_root: path::PathBuf,
    pub next_session_id: u32,
    #[serde(default)]
    pub users: BTreeMap<u32, User>,
    #[serde(default)]
    pub sessions: BTreeMap<String, Session>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Session {
    pub uid: u32,
    pub class: SessionClass,
    pub vt: Option<num::NonZeroU32>,
    pub tty: Option<sync::Arc<str>>,
    pub leader: Option<num::NonZeroU32>,
    pub remote_host: Option<sync::Arc<str>>,
    pub timestamp: u128,
    pub env: BTreeMap<sync::Arc<str>, sync::Arc<str>>,
    #[serde(alias = "active")]
    pub state: SessionState,
    pub recoverable: bool,
    pub display: u32,
}

pub type Result<T> = result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("state file I/O failed: {0}")]
    Io(#[from] io::Error),

    #[error("invalid state file: {0}")]
    Decode(#[from] de::Error),

    #[error("failed to serialize state: {0}")]
    Encode(#[from] ser::Error),
}

impl State {
    #[must_use]
    pub fn path() -> path::PathBuf {
        path::PathBuf::from("/run/sessiond/state.toml")
    }

    /// Watches this state's file without reading or modifying the snapshot.
    ///
    /// Create the monitor before the initial [`Self::try_read`] to avoid missing
    /// updates during that read. The parent directory must already exist.
    ///
    /// # Errors
    /// Returns an error if the inotify instance or directory watch cannot be created.
    #[cfg(feature = "monitor")]
    pub fn monitor() -> io::Result<monitor::Monitor> {
        monitor::Monitor::new(Self::path())
    }

    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Reads session state from a TOML file.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read as UTF-8 or its contents
    /// cannot be deserialized into [`State`].
    pub fn try_read(&mut self) -> Result<()> {
        let contents = fs::read_to_string(Self::path())?;

        *self = toml::from_str::<Self>(&contents)?;
        Ok(())
    }
}
