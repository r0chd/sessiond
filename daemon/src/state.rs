#![allow(clippy::needless_pass_by_value, clippy::type_complexity)]
use crate::ecs;
use anyhow::Context;
use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::{fs, path, time};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct State {
    pub version: u32,
    pub next_session_id: u32,
    #[serde(default)]
    pub sessions: BTreeMap<String, Session>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Session {
    pub uid: u32,
    pub username: String,
    #[serde(default)]
    pub class: String,
    #[serde(default)]
    pub vt: Option<u32>,
    pub leader: i32,
    #[serde(default)]
    pub remote_host: Option<String>,
    pub timestamp: u128,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
}

impl Session {
    pub fn class(&self) -> ecs::SessionClass {
        match self.class.as_str() {
            "greeter" => ecs::SessionClass::Greeter,
            "lock-screen" => ecs::SessionClass::LockScreen,
            "background" => ecs::SessionClass::Background,
            "background-light" => ecs::SessionClass::BackgroundLight,
            "manager" => ecs::SessionClass::Manager,
            "manager-early" => ecs::SessionClass::ManagerEarly,
            _ => ecs::SessionClass::User,
        }
    }

    pub fn timestamp(&self) -> time::SystemTime {
        time::UNIX_EPOCH
            + time::Duration::from_micros(u64::try_from(self.timestamp).unwrap_or(u64::MAX))
    }
}

#[derive(Resource)]
pub struct StateWriter(pub path::PathBuf);

pub fn write_state(
    writer: Res<StateWriter>,
    sessions: Query<
        (
            &ecs::SessionId,
            &ecs::Uid,
            &ecs::Username,
            &ecs::SessionClass,
            Option<&ecs::Vt>,
            &ecs::LeaderId,
            Option<&ecs::RemoteHost>,
            &ecs::Timestamp,
            &ecs::SessionEnv,
        ),
        With<ecs::Session>,
    >,
    changed: Query<
        Entity,
        (
            With<ecs::Session>,
            Or<(
                Added<ecs::Session>,
                Changed<ecs::Uid>,
                Changed<ecs::Username>,
                Changed<ecs::SessionClass>,
                Changed<ecs::Vt>,
                Changed<ecs::LeaderId>,
                Changed<ecs::RemoteHost>,
                Changed<ecs::Timestamp>,
                Changed<ecs::SessionEnv>,
            )>,
        ),
    >,
    removed: RemovedComponents<ecs::Session>,
    next_session_id: Res<ecs::NextSessionId>,
) {
    if changed.is_empty() && removed.is_empty() && !next_session_id.is_changed() {
        return;
    }

    let sessions = sessions
        .iter()
        .map(
            |(id, uid, username, class, vt, leader, remote_host, timestamp, env)| {
                let timestamp = timestamp
                    .0
                    .duration_since(time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_micros();
                let class = match class {
                    ecs::SessionClass::User => "user",
                    ecs::SessionClass::Greeter => "greeter",
                    ecs::SessionClass::LockScreen => "lock-screen",
                    ecs::SessionClass::Background => "background",
                    ecs::SessionClass::BackgroundLight => "background-light",
                    ecs::SessionClass::Manager => "manager",
                    ecs::SessionClass::ManagerEarly => "manager-early",
                };

                (
                    id.0.as_raw().to_string(),
                    Session {
                        uid: uid.0,
                        username: username.0.clone(),
                        class: class.to_owned(),
                        vt: vt.map(|vt| vt.0.get()),
                        leader: leader.0,
                        remote_host: remote_host.map(|host| host.0.clone()),
                        timestamp,
                        env: env.0.clone(),
                    },
                )
            },
        )
        .collect();

    let state = State {
        version: 1,
        next_session_id: next_session_id.0.as_raw(),
        sessions,
    };

    if let Err(error) = state.write(&writer.0) {
        log::error!(error = error.to_string().as_str(); "failed to write session state");
    }
}

impl State {
    pub fn load(path: &path::Path) -> anyhow::Result<Self> {
        let state = fs::read_to_string(path)
            .with_context(|| format!("failed to read state file: {}", path.display()))?;

        toml::from_str::<Self>(&state)
            .inspect_err(|err| {
                log::error!(
                    path = path.to_string_lossy().as_ref(),
                    error = err.to_string().as_str();
                    "failed to parse config file"
                );
            })
            .with_context(|| format!("failed to parse staet file {}", path.display()))
    }

    pub fn write(&self, path: &path::Path) -> anyhow::Result<()> {
        let mut temp_path = path.as_os_str().to_os_string();
        temp_path.push("~");

        let mut file = fs::File::create(&temp_path)?;
        fs::set_permissions(&temp_path, fs::Permissions::from_mode(0o600))?;

        let toml = toml::to_string_pretty(self)?;
        file.write_all(toml.as_bytes())?;
        file.sync_all()?;

        fs::rename(&temp_path, path)?;

        Ok(())
    }
}
