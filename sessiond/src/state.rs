use crate::{cgroup, config, session, user};
use anyhow::Context;
use bevy_ecs::prelude::*;
use hyprwire_protocols_sessiond::server::session_core::v1::session_core_v1;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::{fs, path, sync, time};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct State {
    pub next_session_id: u32,
    #[serde(default)]
    pub sessions: BTreeMap<String, Session>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Session {
    pub uid: u32,
    pub username: sync::Arc<str>,
    #[serde(default)]
    pub class: String,
    #[serde(default)]
    pub vt: Option<u32>,
    pub leader: i32,
    #[serde(default)]
    pub remote_host: Option<sync::Arc<str>>,
    pub timestamp: u128,
    #[serde(default)]
    pub env: BTreeMap<sync::Arc<str>, sync::Arc<str>>,
    #[serde(default)]
    pub active: String,
}

impl Session {
    pub fn class(&self) -> session::SessionClass {
        match self.class.as_str() {
            "greeter" => session::SessionClass::Greeter,
            "lock-screen" => session::SessionClass::LockScreen,
            "background" => session::SessionClass::Background,
            "background-light" => session::SessionClass::BackgroundLight,
            "manager" => session::SessionClass::Manager,
            "manager-early" => session::SessionClass::ManagerEarly,
            _ => session::SessionClass::User,
        }
    }

    pub fn timestamp(&self) -> time::SystemTime {
        time::UNIX_EPOCH
            + time::Duration::from_micros(u64::try_from(self.timestamp).unwrap_or(u64::MAX))
    }

    pub fn active_state(&self) -> session_core_v1::ActiveState {
        match self.active.as_str() {
            "active" => session_core_v1::ActiveState::Active,
            "closing" => session_core_v1::ActiveState::Closing,
            "opening" => session_core_v1::ActiveState::Opening,
            _ => session_core_v1::ActiveState::Online,
        }
    }
}

#[derive(Resource)]
pub struct StateWriter(pub path::PathBuf);

pub fn write_state(
    writer: Res<StateWriter>,
    sessions: Query<
        (
            &session::SessionId,
            &session::Uid,
            &session::Username,
            &session::SessionClass,
            Option<&session::Vt>,
            &session::LeaderId,
            Option<&session::RemoteHost>,
            &session::Timestamp,
            &session::SessionEnv,
            &session::ActiveState,
        ),
        (With<session::Session>, With<session::Recoverable>),
    >,
    changed: Query<
        Entity,
        (
            With<session::Session>,
            Or<(
                Added<session::Session>,
                Changed<session::Uid>,
                Changed<session::Username>,
                Changed<session::SessionClass>,
                Changed<session::Vt>,
                Changed<session::LeaderId>,
                Changed<session::RemoteHost>,
                Changed<session::Timestamp>,
                Changed<session::SessionEnv>,
                Changed<session::ActiveState>,
            )>,
        ),
    >,
    removed: RemovedComponents<session::Session>,
    next_session_id: Res<session::NextSessionId>,
) {
    if changed.is_empty() && removed.is_empty() && !next_session_id.is_changed() {
        return;
    }

    let sessions = sessions
        .iter()
        .map(
            |(id, uid, username, class, vt, leader, remote_host, timestamp, env, active_state)| {
                let timestamp = timestamp
                    .0
                    .duration_since(time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_micros();
                let class = match class {
                    session::SessionClass::User => "user",
                    session::SessionClass::Greeter => "greeter",
                    session::SessionClass::LockScreen => "lock-screen",
                    session::SessionClass::Background => "background",
                    session::SessionClass::BackgroundLight => "background-light",
                    session::SessionClass::Manager => "manager",
                    session::SessionClass::ManagerEarly => "manager-early",
                };
                let active = match active_state.0 {
                    session_core_v1::ActiveState::Active => "active",
                    session_core_v1::ActiveState::Closing => "closing",
                    session_core_v1::ActiveState::Opening => "opening",
                    _ => "online",
                };

                (
                    id.as_raw().to_string(),
                    Session {
                        uid: uid.0,
                        username: sync::Arc::clone(&username.0),
                        class: class.to_owned(),
                        vt: vt.map(|vt| vt.0.get()),
                        leader: leader.0,
                        remote_host: remote_host.map(|host| sync::Arc::clone(&host.0)),
                        timestamp,
                        env: env
                            .0
                            .iter()
                            .map(|(name, value)| (sync::Arc::clone(name), sync::Arc::clone(value)))
                            .collect(),
                        active: active.to_owned(),
                    },
                )
            },
        )
        .collect();

    let state = State {
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

pub fn load_state(
    mut commands: Commands,
    state_writer: Res<StateWriter>,
    mut next_session_id: ResMut<session::NextSessionId>,
    cgroup_manager: Res<cgroup::CgroupManager>,
    config: Res<config::Config>,
    mut session_registry: ResMut<session::SessionRegistry>,
) {
    if state_writer.0.exists() {
        match State::load(&state_writer.0) {
            Ok(saved_state) => {
                next_session_id.0 = session::SessionId::from_raw(saved_state.next_session_id);

                for (raw_id, saved_session) in saved_state.sessions {
                    let Ok(raw_id) = raw_id.parse::<u32>() else {
                        log::warn!(session_id = raw_id.as_str(); "ignoring invalid session ID in state");
                        continue;
                    };
                    let session_id = session::SessionId::from_raw(raw_id);

                    let cgroup = match cgroup_manager
                        .restore_session_cgroup(saved_session.uid, session_id)
                    {
                        Ok(cgroup) => cgroup,
                        Err(error) => {
                            log::warn!(session_id = raw_id, error = error.to_string().as_str(); "ignoring session with an unusable cgroup");
                            continue;
                        }
                    };

                    let runtime_dir = config
                        .users
                        .get(&saved_session.username)
                        .cloned()
                        .unwrap_or_default()
                        .runtime_dir(saved_session.uid);
                    user::create_runtime_dir(saved_session.uid, &runtime_dir);

                    let active_state = saved_session.active_state();

                    let entity = session::spawn(
                        &mut commands,
                        session_id,
                        saved_session.class(),
                        saved_session.uid,
                        sync::Arc::clone(&saved_session.username),
                        saved_session.vt,
                        saved_session.leader,
                        saved_session.remote_host.as_ref().map(sync::Arc::clone),
                        active_state,
                        None,
                        true,
                        saved_session.env.clone(),
                        cgroup,
                        &mut session_registry,
                    );
                    commands
                        .entity(entity)
                        .insert(session::Timestamp(saved_session.timestamp()));
                }
            }
            Err(error) => {
                log::error!(error = error.to_string().as_str(); "failed to load session state");
            }
        }
    }
}
