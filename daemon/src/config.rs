use anyhow::Context;
use std::collections::BTreeMap;
use std::{fs, io, path};

#[derive(serde::Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum SeatKind {
    #[default]
    VtBound,
    NonVt,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SeatConfig {
    #[serde(default = "default_user")]
    pub user: String,
    #[serde(default = "default_group")]
    pub group: String,
    #[serde(default)]
    pub kind: SeatKind,
}

impl Default for SeatConfig {
    fn default() -> Self {
        Self {
            user: "root".to_string(),
            group: "root".to_string(),
            kind: SeatKind::VtBound,
        }
    }
}

fn default_user() -> String {
    "root".to_string()
}

fn default_group() -> String {
    "root".to_string()
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagementConfig {
    #[serde(default = "default_user")]
    pub user: String,
    #[serde(default = "default_group")]
    pub group: String,
}

impl Default for ManagementConfig {
    fn default() -> Self {
        Self {
            user: "root".to_string(),
            group: "root".to_string(),
        }
    }
}

#[derive(serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct UserConfig {
    #[serde(default)]
    pub linger: bool,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Config {
    #[serde(default)]
    pub management: ManagementConfig,
    #[serde(default)]
    pub users: BTreeMap<String, UserConfig>,
    #[serde(default = "default_seats")]
    pub seats: BTreeMap<String, SeatConfig>,
    #[serde(default = "default_seat")]
    pub default_seat: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            management: ManagementConfig::default(),
            users: BTreeMap::new(),
            seats: default_seats(),
            default_seat: "seat0".to_string(),
        }
    }
}

fn default_seats() -> BTreeMap<String, SeatConfig> {
    BTreeMap::from([("seat0".to_string(), SeatConfig::default())])
}

fn default_seat() -> String {
    "seat0".to_string()
}

impl Config {
    pub fn load(path: Option<path::PathBuf>) -> anyhow::Result<Self> {
        let explicit_path = path.is_some();
        let path = path.unwrap_or_else(|| path::PathBuf::from("/etc/sessiond.toml"));

        let config = match fs::read_to_string(&path) {
            Ok(config) => config,
            Err(err) if !explicit_path && err.kind() == io::ErrorKind::NotFound => {
                log::info!(path = path.to_string_lossy().as_ref(); "config file not found, using defaults");
                return Ok(Self::default());
            }
            Err(err) => {
                log::error!(
                    path = path.to_string_lossy().as_ref(),
                    error = err.to_string().as_str();
                    "failed to read config file"
                );
                return Err(err)
                    .with_context(|| format!("failed to read config file {}", path.display()));
            }
        };

        toml::from_str::<Self>(&config)
            .inspect_err(|err| {
                log::error!(
                    path = path.to_string_lossy().as_ref(),
                    error = err.to_string().as_str();
                    "failed to parse config file"
                );
            })
            .with_context(|| format!("failed to parse config file {}", path.display()))
    }
}
