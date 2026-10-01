use crate::{cgroup, config, session, user};
use bevy_ecs::prelude::*;
use hyprwire_protocols_sessiond::server::session_core::v1::session_core_v1;
use sessiond_state::{Session, SessionClass, SessionState, State, UserState};
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::{fs, path, sync, time};

trait SessionExt {
    fn class(&self) -> session::SessionClass;
    fn timestamp(&self) -> time::SystemTime;
    fn session_state(&self) -> session_core_v1::SessionState;
}

impl SessionExt for Session {
    fn class(&self) -> session::SessionClass {
        match self.class {
            SessionClass::Greeter => session::SessionClass::Greeter,
            SessionClass::LockScreen => session::SessionClass::LockScreen,
            SessionClass::Background => session::SessionClass::Background,
            SessionClass::BackgroundLight => session::SessionClass::BackgroundLight,
            SessionClass::Manager => session::SessionClass::Manager,
            SessionClass::ManagerEarly => session::SessionClass::ManagerEarly,
            SessionClass::User | SessionClass::Unknown => session::SessionClass::User,
        }
    }

    fn timestamp(&self) -> time::SystemTime {
        time::UNIX_EPOCH
            + time::Duration::from_micros(u64::try_from(self.timestamp).unwrap_or(u64::MAX))
    }

    fn session_state(&self) -> session_core_v1::SessionState {
        match self.state {
            SessionState::Active => session_core_v1::SessionState::Active,
            SessionState::Closing => session_core_v1::SessionState::Closing,
            SessionState::Opening => session_core_v1::SessionState::Opening,
            SessionState::Online | SessionState::Unknown => session_core_v1::SessionState::Online,
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
            &session::SessionClass,
            Option<&session::Vt>,
            &session::Leader,
            Option<&session::RemoteHost>,
            &session::Timestamp,
            &session::SessionEnv,
            &session::SessionState,
            &session::DisplaySession,
            Option<&session::Recoverable>,
        ),
        With<session::Session>,
    >,
    next_session_id: Res<session::NextSessionId>,
    users: Query<(&session::Uid, &user::UserState), With<user::User>>,
) {
    let mut saved_state = State::new();
    saved_state.next_session_id = next_session_id.0.as_raw();

    for (
        id,
        uid,
        class,
        vt,
        leader,
        remote_host,
        timestamp,
        env,
        session_state,
        display,
        recoverable,
    ) in &sessions
    {
        let timestamp = timestamp
            .0
            .duration_since(time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros();
        let class = match class {
            session::SessionClass::User => SessionClass::User,
            session::SessionClass::Greeter => SessionClass::Greeter,
            session::SessionClass::LockScreen => SessionClass::LockScreen,
            session::SessionClass::Background => SessionClass::Background,
            session::SessionClass::BackgroundLight => SessionClass::BackgroundLight,
            session::SessionClass::Manager => SessionClass::Manager,
            session::SessionClass::ManagerEarly => SessionClass::ManagerEarly,
        };
        let state = match session_state.0 {
            session_core_v1::SessionState::Active => SessionState::Active,
            session_core_v1::SessionState::Closing => SessionState::Closing,
            session_core_v1::SessionState::Opening => SessionState::Opening,
            _ => SessionState::Online,
        };

        saved_state.sessions.insert(
            id.as_raw().to_string(),
            Session {
                uid: uid.0,
                class,
                vt: vt.map(|vt| vt.0.get()),
                leader: leader.0,
                remote_host: remote_host.map(|host| sync::Arc::clone(&host.0)),
                timestamp,
                env: env
                    .0
                    .iter()
                    .map(|(name, value)| (sync::Arc::clone(name), sync::Arc::clone(value)))
                    .collect(),
                state,
                recoverable: recoverable.is_some(),
                display: display.0.as_raw(),
            },
        );
    }

    for (uid, state) in &users {
        let state = match state.0 {
            session_core_v1::UserState::Active => UserState::Active,
            session_core_v1::UserState::Closing => UserState::Closing,
            session_core_v1::UserState::Lingering => UserState::Lingering,
            session_core_v1::UserState::Online => UserState::Online,
            session_core_v1::UserState::Offline => UserState::Offline,
            _ => UserState::Unknown,
        };
        saved_state
            .users
            .insert(uid.0, sessiond_state::User { state });
    }

    if let Err(error) = saved_state.write(&writer.0) {
        log::error!(error = error.to_string().as_str(); "failed to write session state");
    }
}

trait StateExt: Sized {
    fn write(&self, path: &path::Path) -> anyhow::Result<()>;
}

impl StateExt for State {
    fn write(&self, path: &path::Path) -> anyhow::Result<()> {
        let mut temp_path = path.as_os_str().to_os_string();
        temp_path.push("~");

        let mut file = fs::File::create(&temp_path)?;
        fs::set_permissions(&temp_path, fs::Permissions::from_mode(0o644))?;

        let toml = toml::to_string_pretty(self)?;
        file.write_all(toml.as_bytes())?;
        file.sync_all()?;

        fs::rename(&temp_path, path)?;

        Ok(())
    }
}

pub fn load_state(
    mut commands: Commands,
    sessions: Query<
        (&session::SessionId, &session::Uid, &session::Timestamp),
        With<session::Session>,
    >,
    mut next_session_id: ResMut<session::NextSessionId>,
    cgroup_manager: Res<cgroup::CgroupManager>,
    config: Res<config::Config>,
    mut session_registry: ResMut<session::SessionRegistry>,
    loop_handle: NonSend<'_, calloop::LoopHandle<'static, crate::Sessiond>>,
) {
    let mut state = sessiond_state::State::new();
    match state.try_read() {
        Ok(()) => {
            next_session_id.0 = session::SessionId::from_raw(state.next_session_id);

            for (raw_id, saved_session) in state.sessions {
                if !saved_session.recoverable {
                    continue;
                }

                let Ok(raw_id) = raw_id.parse::<u32>() else {
                    log::warn!(session_id = raw_id.as_str(); "ignoring invalid session ID in state");
                    continue;
                };
                let session_id = session::SessionId::from_raw(raw_id);

                let Some(user) = uzers::get_user_by_uid(saved_session.uid) else {
                    log::warn!(session_id = raw_id, uid = saved_session.uid; "ignoring session whose user no longer exists");
                    continue;
                };
                let username: sync::Arc<str> = user.name().to_string_lossy().into_owned().into();

                let cgroup = match cgroup_manager
                    .restore_session_cgroup(saved_session.uid, session_id)
                {
                    Ok(cgroup) => cgroup,
                    Err(error) => {
                        log::warn!(session_id = raw_id, error = error.to_string().as_str(); "ignoring session with an unusable cgroup");
                        continue;
                    }
                };
                if let Err(error) = cgroup::watch_empty(
                    &cgroup.path,
                    session_id,
                    session::Uid(saved_session.uid),
                    &loop_handle,
                ) {
                    log::warn!(session_id = raw_id, error = error.to_string().as_str(); "failed to watch recovered session cgroup");
                    continue;
                }

                let runtime_dir = config
                    .users
                    .get(&username)
                    .cloned()
                    .unwrap_or_default()
                    .runtime_dir(saved_session.uid);
                user::create_runtime_dir(saved_session.uid, &runtime_dir);

                let session_state = saved_session.session_state();

                let display_session = sessions
                    .iter()
                    .filter(|(_, uid, _)| uid.0 == saved_session.uid)
                    .min_by_key(|(session_id, _, timestamp)| (timestamp.0, session_id.as_raw()))
                    .map_or(session_id, |(session_id, _, _)| *session_id);

                let entity = session::spawn(
                    &mut commands,
                    session_id,
                    saved_session.class(),
                    saved_session.uid,
                    saved_session.vt,
                    saved_session.leader,
                    saved_session.remote_host.as_ref().map(sync::Arc::clone),
                    session_state,
                    None,
                    true,
                    saved_session.env.clone(),
                    cgroup,
                    &mut session_registry,
                    display_session,
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
