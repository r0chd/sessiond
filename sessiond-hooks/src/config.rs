use anyhow::Context as _;
use serde::Deserialize;
use std::{fs, path};

#[derive(Clone, Copy, Default, Deserialize)]
pub enum RestartPolicy {
    #[default]
    Failure,
    Never,
    Always,
}

#[derive(Deserialize)]
pub struct Restart {
    pub on: RestartPolicy,
    pub delay: u64,
}

impl Default for Restart {
    fn default() -> Self {
        Self {
            on: RestartPolicy::default(),
            delay: 5,
        }
    }
}

#[derive(Deserialize)]
pub struct Config {
    pub on_start: Command,
    pub restart: Restart,
}

#[derive(Clone, Deserialize)]
pub struct Command {
    pub exec: String,
    pub args: Vec<String>,
}

impl Command {
    pub fn expand_home(&self, home: &str) -> Self {
        fn expand(value: &str, home: &str) -> String {
            if value == "~" {
                return home.to_owned();
            }

            value.strip_prefix("~/").map_or_else(
                || value.to_owned(),
                |relative| format!("{}/{relative}", home.trim_end_matches('/')),
            )
        }

        Self {
            exec: expand(&self.exec, home),
            args: self.args.iter().map(|arg| expand(arg, home)).collect(),
        }
    }
}

impl Config {
    pub fn load(path: Option<path::PathBuf>) -> anyhow::Result<Self> {
        let path = path.unwrap_or_else(|| path::PathBuf::from("/etc/sessiond-hooks.toml"));

        let config = match fs::read_to_string(&path) {
            Ok(config) => config,
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
