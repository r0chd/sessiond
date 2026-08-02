use anyhow::Context;
use std::collections::BTreeMap;
use std::{fs, io, path};
use uzers::os::unix::UserExt;

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

#[derive(Clone, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct UserConfig {
    #[serde(default)]
    pub linger: bool,
    #[serde(default = "default_runtime_dir")]
    pub runtime_dir: String,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
}

impl Default for UserConfig {
    fn default() -> Self {
        Self {
            linger: false,
            runtime_dir: default_runtime_dir(),
            env: BTreeMap::new(),
        }
    }
}

impl UserConfig {
    pub fn runtime_dir(&self, uid: u32) -> String {
        self.runtime_dir
            .replace(concat!("{", "uid", "}"), &uid.to_string())
    }

    pub fn environment(
        &self,
        uid: u32,
        session_id: u32,
        term: Option<&str>,
    ) -> BTreeMap<String, String> {
        let runtime_dir = self.runtime_dir(uid);
        let mut env = BTreeMap::from([
            ("XDG_RUNTIME_DIR".to_string(), runtime_dir.clone()),
            ("XDG_SESSION_ID".to_string(), session_id.to_string()),
            ("TERM".to_string(), term.unwrap_or("linux").to_string()),
        ]);

        if let Some(user) = uzers::get_user_by_uid(uid) {
            let username = user.name().to_string_lossy().to_string();
            env.insert("USER".to_string(), username.clone());
            env.insert("LOGNAME".to_string(), username);
            env.insert(
                "HOME".to_string(),
                user.home_dir().to_string_lossy().to_string(),
            );
            env.insert(
                "SHELL".to_string(),
                user.shell().to_string_lossy().to_string(),
            );
        }

        for (name, value) in &self.env {
            env.insert(
                name.clone(),
                value
                    .replace(concat!("{", "uid", "}"), &uid.to_string())
                    .replace(concat!("{", "runtime-dir", "}"), &runtime_dir),
            );
        }

        env
    }
}

fn default_runtime_dir() -> String {
    "/run/user/{uid}".to_string()
}

#[derive(serde::Deserialize, Default)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct PowerConfig {
    #[serde(default)]
    pub reboot: Vec<String>,
    #[serde(default)]
    pub poweroff: Vec<String>,
    #[serde(default)]
    pub suspend: Vec<String>,
    #[serde(default)]
    pub hibernate: Vec<String>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
#[derive(Default)]
pub struct Config {
    #[serde(default)]
    pub management: ManagementConfig,
    #[serde(default)]
    pub users: BTreeMap<String, UserConfig>,
    #[serde(default)]
    pub power: PowerConfig,
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
