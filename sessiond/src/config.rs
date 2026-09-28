use anyhow::Context;
use bevy_ecs::prelude::*;
use std::collections::BTreeMap;
use std::{fs, io, path, sync};

#[derive(Clone, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct UserConfig {
    #[serde(default)]
    pub linger: bool,
    #[serde(default = "default_runtime_dir")]
    pub runtime_dir: String,
}

impl Default for UserConfig {
    fn default() -> Self {
        Self {
            linger: false,
            runtime_dir: default_runtime_dir(),
        }
    }
}

impl UserConfig {
    pub fn runtime_dir(&self, uid: u32) -> sync::Arc<str> {
        self.runtime_dir
            .replace(concat!("{", "uid", "}"), &uid.to_string())
            .into()
    }
}

fn default_runtime_dir() -> String {
    "/run/user/{uid}".to_string()
}

#[derive(serde::Deserialize, Resource)]
#[serde(rename_all = "kebab-case")]
#[derive(Default)]
pub struct Config {
    #[serde(default)]
    pub users: BTreeMap<sync::Arc<str>, UserConfig>,
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
