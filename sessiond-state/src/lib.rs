use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::{fs, io, path, result, sync};
use toml::{de, ser};

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
pub enum ActiveState {
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

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct State {
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
    #[serde(default)]
    pub class: SessionClass,
    #[serde(default)]
    pub vt: Option<u32>,
    pub leader: i32,
    #[serde(default)]
    pub remote_host: Option<sync::Arc<str>>,
    pub timestamp: u128,
    #[serde(default)]
    pub env: BTreeMap<sync::Arc<str>, sync::Arc<str>>,
    #[serde(default)]
    pub active: ActiveState,
    pub recoverable: bool,
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

/// Reads session state from a TOML file.
///
/// # Errors
///
/// Returns an error if the file cannot be read as UTF-8 or its contents
/// cannot be deserialized into [`State`].
pub fn try_read(path: &path::Path) -> Result<State> {
    let state = fs::read_to_string(path)?;

    Ok(toml::from_str::<State>(&state)?)
}
