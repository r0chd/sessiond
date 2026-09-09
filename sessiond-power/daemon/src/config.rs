use anyhow::Context;
use serde::Deserialize;
use std::{fs, io, path};

const DEFAULT_CONFIG_PATH: &str = "/etc/sessiond-power.toml";

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
pub struct Config {
    pub reboot: Vec<String>,
    pub poweroff: Vec<String>,
    pub suspend: Vec<String>,
    pub hibernate: Vec<String>,
    pub hybrid_sleep: Vec<String>,
}

impl Config {
    pub fn load(path: Option<path::PathBuf>) -> anyhow::Result<Self> {
        let explicit_path = path.is_some();
        let path = path.unwrap_or_else(|| path::PathBuf::from(DEFAULT_CONFIG_PATH));

        let contents = match fs::read_to_string(&path) {
            Ok(contents) => contents,
            Err(error) if !explicit_path && error.kind() == io::ErrorKind::NotFound => {
                return Ok(Self::default());
            }
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("failed to read config from {}", path.display()));
            }
        };

        toml::from_str(&contents)
            .with_context(|| format!("failed to parse config from {}", path.display()))
    }
}
