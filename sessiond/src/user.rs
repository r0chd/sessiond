use crate::{cgroup, config, session};
use bevy_ecs::prelude::*;
use hyprwire_protocols_sessiond::server::session_core::v1::session_core_v1;
use hyprwire_protocols_sessiond::server::session_core::v1::session_core_v1::session_core_user;
use std::collections::HashMap;
use std::{fs, io, sync};

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct UserState(pub session_core_v1::UserState);

impl UserState {
    pub const ACTIVE: Self = Self(session_core_v1::UserState::Active);
    pub const CLOSING: Self = Self(session_core_v1::UserState::Closing);
    pub const LINGERING: Self = Self(session_core_v1::UserState::Lingering);
    pub const OFFLINE: Self = Self(session_core_v1::UserState::Offline);
    pub const ONLINE: Self = Self(session_core_v1::UserState::Online);
}

impl From<session_core_v1::UserState> for UserState {
    fn from(state: session_core_v1::UserState) -> Self {
        Self(state)
    }
}

#[derive(Resource, Default)]
pub struct UserRegistry {
    pub by_uid: HashMap<u32, Entity>,
    pub by_object: HashMap<session_core_user::SessionCoreUser, Entity>,
}

#[derive(Bundle)]
pub struct UserBundle {
    pub marker: User,
    pub uid: session::Uid,
    pub username: session::Username,
    pub state: UserState,
    pub objects: Objects,
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct User;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lingering;

#[derive(Component, Default)]
pub struct Objects(pub Vec<session_core_user::SessionCoreUser>);

pub fn spawn(
    commands: &mut Commands,
    uid: u32,
    username: sync::Arc<str>,
    lingering: bool,
    state: UserState,
    user_registry: &mut ResMut<UserRegistry>,
    info_managers: &session::SessionInfoManagers,
) -> Entity {
    let mut cmd = commands.spawn(UserBundle {
        marker: User,
        uid: session::Uid(uid),
        username: session::Username(username),
        state,
        objects: Objects::default(),
    });

    if lingering {
        cmd.insert(Lingering);
    }

    let entity = cmd.id();
    user_registry.by_uid.insert(uid, entity);
    for manager in &info_managers.0 {
        manager.send_user_added(uid);
    }
    entity
}

pub fn create_runtime_dir(uid: u32, path: &str) {
    if let Err(e) = fs::create_dir_all(path) {
        log::error!(uid, error = e.to_string().as_str(); "failed to create runtime dir");
    }
    if let Err(e) = rustix::fs::chown(path, Some(rustix::fs::Uid::from_raw(uid)), None) {
        log::error!(uid, error = e.to_string().as_str(); "failed to chown runtime dir");
    }
    if let Err(e) = rustix::fs::chmod(path, rustix::fs::Mode::from_bits_truncate(0o700)) {
        log::error!(uid, error = e.to_string().as_str(); "failed to chmod runtime dir");
    }
}

pub fn remove_runtime_dir(path: &str) -> io::Result<()> {
    match fs::remove_dir(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

pub fn ensure_session_users(
    mut commands: Commands,
    sessions: Query<
        (&session::Uid, &session::Username, &session::ActiveState),
        (With<session::Session>, Added<session::Session>),
    >,
    mut user_registry: ResMut<UserRegistry>,
    info_managers: Res<session::SessionInfoManagers>,
) {
    for (uid, username, session_state) in sessions {
        if user_registry.by_uid.contains_key(&uid.0) {
            continue;
        }

        let state = match session_state.0 {
            session_core_v1::ActiveState::Active => UserState::ACTIVE,
            session_core_v1::ActiveState::Closing => UserState::CLOSING,
            _ => UserState::ONLINE,
        };

        spawn(
            &mut commands,
            uid.0,
            sync::Arc::clone(&username.0),
            false,
            state,
            &mut user_registry,
            &info_managers,
        );
    }
}

pub fn reconcile_user_states(
    users: Query<(&session::Uid, Option<&Lingering>, &mut UserState), With<User>>,
    sessions: Query<
        (&session::Uid, &session::SessionClass, &session::ActiveState),
        With<session::Session>,
    >,
) {
    for (uid, lingering, mut user_state) in users {
        let states: Vec<_> = sessions
            .iter()
            .filter(|(session_uid, class, _)| {
                session_uid.0 == uid.0
                    && !matches!(
                        class,
                        session::SessionClass::Manager | session::SessionClass::ManagerEarly
                    )
            })
            .map(|(_, _, state)| state)
            .collect();

        let desired = if states.contains(&&session::ActiveState::ACTIVE) {
            UserState::ACTIVE
        } else if !states.is_empty()
            && states
                .iter()
                .all(|state| **state == session::ActiveState::CLOSING)
        {
            UserState::CLOSING
        } else if !states.is_empty() {
            UserState::ONLINE
        } else if lingering.is_some() {
            UserState::LINGERING
        } else {
            UserState::OFFLINE
        };

        if *user_state != desired {
            *user_state = desired;
        }
    }
}

#[derive(Message)]
pub struct AddObject {
    pub uid: u32,
    pub object: session_core_user::SessionCoreUser,
    pub creds: hyprwire::Creds,
}

pub fn handle_add_object(
    mut requests: MessageReader<AddObject>,
    mut users: Query<
        (
            Entity,
            &session::Uid,
            &session::Username,
            Option<&Lingering>,
            &UserState,
            &mut Objects,
        ),
        With<User>,
    >,
    mut user_registry: ResMut<UserRegistry>,
) {
    for request in requests.read() {
        let Some((entity, uid, username, lingering, state, mut objects)) = users
            .iter_mut()
            .find(|(_, uid, _, _, _, _)| uid.0 == request.uid)
        else {
            log::warn!(uid = request.uid; "get_user requested for unknown user");
            request.object.send_not_found();
            continue;
        };

        request.object.send_uid(uid.0);
        request.object.send_username(&username.0);
        if lingering.is_some() {
            request.object.send_lingering(1);
        } else {
            request.object.send_lingering(0);
        }
        request.object.send_state(state.0);

        request.object.send_can_modify(u32::from(
            request.creds.uid.is_root() || uid.0 == request.creds.uid.as_raw(),
        ));
        request.object.send_done();

        objects.0.push(request.object.clone());
        user_registry
            .by_object
            .insert(request.object.clone(), entity);
    }
}

#[derive(Message)]
pub struct UserControlMessage {
    pub object: session_core_user::SessionCoreUser,
    pub action: UserControlAction,
}

pub enum UserControlAction {
    Terminate { creds: hyprwire::Creds },
    Kill { creds: hyprwire::Creds },
    Destroy,
}

pub fn handle_user_control(
    mut actions: MessageReader<UserControlMessage>,
    mut session_actions: MessageWriter<session::SessionControlMessage>,
    mut user_registry: ResMut<UserRegistry>,
    mut users: Query<(&session::Uid, &mut Objects), With<User>>,
    sessions: Query<(Entity, &session::Uid), With<session::Session>>,
) {
    for action in actions.read() {
        let Some(user_entity) = user_registry.by_object.get(&action.object).copied() else {
            continue;
        };

        match action.action {
            UserControlAction::Terminate { creds } => {
                let Ok((uid, _)) = users.get(user_entity) else {
                    continue;
                };
                if !creds.uid.is_root() && uid.0 != creds.uid.as_raw() {
                    action.object.send_error(
                        session_core_v1::ErrorCode::PermissionDenied,
                        "permission denied",
                    );
                    continue;
                }

                for (session_entity, _) in sessions
                    .iter()
                    .filter(|(_, session_uid)| *session_uid == uid)
                {
                    session_actions.write(session::SessionControlMessage {
                        entity: session_entity,
                        object: None,
                        action: session::SessionControlAction::Terminate { creds },
                    });
                }

                action.object.send_ok();
            }
            UserControlAction::Kill { creds } => {
                let Ok((uid, _)) = users.get(user_entity) else {
                    continue;
                };
                if !creds.uid.is_root() && uid.0 != creds.uid.as_raw() {
                    action.object.send_error(
                        session_core_v1::ErrorCode::PermissionDenied,
                        "permission denied",
                    );
                    continue;
                }

                for (session_entity, _) in sessions
                    .iter()
                    .filter(|(_, session_uid)| *session_uid == uid)
                {
                    session_actions.write(session::SessionControlMessage {
                        entity: session_entity,
                        object: None,
                        action: session::SessionControlAction::Kill { creds },
                    });
                }

                action.object.send_ok();
            }
            UserControlAction::Destroy => {
                user_registry.by_object.remove(&action.object);
                if let Ok((_, mut objects)) = users.get_mut(user_entity) {
                    objects.0.retain(|candidate| *candidate != action.object);
                }
            }
        }
    }
}

impl hyprwire::Dispatch<session_core_user::SessionCoreUser> for crate::Sessiond {
    fn event(
        &mut self,
        object: &session_core_user::SessionCoreUser,
        event: <session_core_user::SessionCoreUser as hyprwire::Object>::Event<'_>,
    ) {
        if !self
            .world
            .resource::<UserRegistry>()
            .by_object
            .contains_key(object)
        {
            return;
        }
        let client = object.client();

        match event {
            session_core_user::Event::Destroy => {
                self.world.write_message(UserControlMessage {
                    object: object.clone(),
                    action: UserControlAction::Destroy,
                });
            }
            session_core_user::Event::Terminate => {
                self.world.write_message(UserControlMessage {
                    object: object.clone(),
                    action: UserControlAction::Terminate {
                        creds: *client.creds(),
                    },
                });
            }
            session_core_user::Event::Kill => {
                self.world.write_message(UserControlMessage {
                    object: object.clone(),
                    action: UserControlAction::Kill {
                        creds: *client.creds(),
                    },
                });
            }
            _ => {}
        }
    }
}

pub fn initialize_lingering_users(
    mut commands: Commands,
    config: Res<config::Config>,
    cgroup_manager: Res<cgroup::CgroupManager>,
    mut user_registry: ResMut<UserRegistry>,
    info_managers: Res<session::SessionInfoManagers>,
) {
    let lingering_users = config
        .users
        .iter()
        .filter(|(_, config)| config.linger)
        .filter_map(|(name, config)| {
            let user = uzers::get_user_by_name(name.as_ref())?;
            let uid = user.uid();
            Some((uid, config.runtime_dir(uid)))
        })
        .collect::<Vec<_>>();
    for (uid, runtime_dir) in lingering_users {
        if let Some(user) = uzers::get_user_by_uid(uid) {
            spawn(
                &mut commands,
                uid,
                user.name().to_string_lossy().into(),
                true,
                UserState::LINGERING,
                &mut user_registry,
                &info_managers,
            );

            create_runtime_dir(uid, &runtime_dir);
            if let Err(e) = cgroup_manager.ensure_user_cgroup(uid) {
                log::error!(
                    uid,
                    error = e.to_string().as_str();
                    "failed to create user cgroup"
                );
            }
        } else {
            log::warn!(uid; "configured lingering user no longer exists");
        }
    }
}
