use crate::{access, session, session_core_user, session_core_v1};
use bevy_ecs::prelude::*;
use bevy_ecs::{entity, world};
use std::collections::HashMap;
use std::{fs, io};

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
    pub state: session_core_v1::UserState,
    pub objects: Objects,
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct User;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lingering;

#[derive(Component, Default)]
pub struct Objects(pub Vec<session_core_user::SessionCoreUser>);

pub fn spawn(
    world: &mut world::World,
    uid: u32,
    username: String,
    lingering: bool,
    state: session_core_v1::UserState,
) -> entity::Entity {
    let mut cmd = world.spawn(UserBundle {
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
    world
        .resource_mut::<UserRegistry>()
        .by_uid
        .insert(uid, entity);
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

pub fn create(world: &mut world::World, uid: u32, lingering: bool) -> Option<entity::Entity> {
    let user = uzers::get_user_by_uid(uid)?;
    let states = world
        .resource::<session::SessionRegistry>()
        .entities_by_uid(uid)
        .into_iter()
        .filter_map(|entity| world.get::<session_core_v1::ActiveState>(entity).copied())
        .collect::<Vec<_>>();

    let state = if states.contains(&session_core_v1::ActiveState::Active) {
        session_core_v1::UserState::Active
    } else if !states.is_empty() {
        if states
            .iter()
            .all(|state| *state == session_core_v1::ActiveState::Closing)
        {
            session_core_v1::UserState::Closing
        } else {
            session_core_v1::UserState::Online
        }
    } else if lingering {
        session_core_v1::UserState::Lingering
    } else {
        session_core_v1::UserState::Offline
    };

    Some(spawn(
        world,
        uid,
        user.name().to_string_lossy().into_owned(),
        lingering,
        state,
    ))
}

pub fn reconcile_user_states(
    users: Query<
        (
            &session::Uid,
            Option<&Lingering>,
            &mut session_core_v1::UserState,
        ),
        With<User>,
    >,
    sessions: Query<(&session::Uid, &session_core_v1::ActiveState), With<session::Session>>,
) {
    for (uid, lingering, mut user_state) in users {
        let states: Vec<_> = sessions
            .iter()
            .filter(|(session_uid, _)| session_uid.0 == uid.0)
            .map(|(_, state)| *state)
            .collect();

        let desired = if states.contains(&session_core_v1::ActiveState::Active) {
            session_core_v1::UserState::Active
        } else if !states.is_empty()
            && states
                .iter()
                .all(|state| *state == session_core_v1::ActiveState::Closing)
        {
            session_core_v1::UserState::Closing
        } else if !states.is_empty() {
            session_core_v1::UserState::Online
        } else if lingering.is_some() {
            session_core_v1::UserState::Lingering
        } else {
            session_core_v1::UserState::Offline
        };

        if *user_state != desired {
            *user_state = desired;
        }
    }
}

pub fn add_object(
    world: &mut world::World,
    entity: entity::Entity,
    object: session_core_user::SessionCoreUser,
    management_access: access::Access,
) {
    if let Some(uid) = world.get::<session::Uid>(entity) {
        object.send_uid(uid.0);
    }
    if let Some(username) = world.get::<session::Username>(entity) {
        object.send_username(&username.0);
    }
    if world.get::<Lingering>(entity).is_some() {
        object.send_lingering(1);
    } else {
        object.send_lingering(0);
    }
    if let Some(state) = world.get::<session_core_v1::UserState>(entity) {
        object.send_state(*state);
    }
    let client = object.client();
    let uid = world.get::<session::Uid>(entity).map_or(0, |uid| uid.0);
    let can_modify = management_access.allows(client.creds()) || uid == client.creds().uid.as_raw();
    object.send_can_modify(u32::from(can_modify));
    object.send_done();

    if let Some(mut objects) = world.get_mut::<Objects>(entity) {
        objects.0.push(object.clone());
    }
    world
        .resource_mut::<UserRegistry>()
        .by_object
        .insert(object, entity);
}

impl hyprwire::Dispatch<session_core_user::SessionCoreUser> for crate::Sessiond {
    fn event(
        &mut self,
        object: &session_core_user::SessionCoreUser,
        event: <session_core_user::SessionCoreUser as hyprwire::Object>::Event<'_>,
    ) {
        let Some(entity) = self
            .world
            .resource::<UserRegistry>()
            .by_object
            .get(object)
            .copied()
        else {
            return;
        };
        let client = object.client();

        match event {
            session_core_user::Event::Destroy => {
                self.world
                    .resource_mut::<UserRegistry>()
                    .by_object
                    .remove(object);
                if let Some(mut objects) = self.world.get_mut::<Objects>(entity) {
                    objects.0.retain(|candidate| candidate != object);
                }
            }
            session_core_user::Event::Terminate => {
                let uid = self
                    .world
                    .get::<session::Uid>(entity)
                    .map_or(0, |uid| uid.0);
                if !self
                    .world
                    .resource::<access::AccessRules>()
                    .management
                    .allows(client.creds())
                    && uid != client.creds().uid.as_raw()
                {
                    object.send_error(
                        session_core_v1::ErrorCode::PermissionDenied,
                        "permission denied",
                    );
                    return;
                }

                let entities = self
                    .world
                    .resource::<session::SessionRegistry>()
                    .entities_by_uid(uid);
                for entity in entities {
                    self.terminate_session(entity);
                }

                object.send_ok();
            }
            session_core_user::Event::Kill => {
                let uid = self
                    .world
                    .get::<session::Uid>(entity)
                    .map_or(0, |uid| uid.0);
                if !self
                    .world
                    .resource::<access::AccessRules>()
                    .management
                    .allows(client.creds())
                    && uid != client.creds().uid.as_raw()
                {
                    object.send_error(
                        session_core_v1::ErrorCode::PermissionDenied,
                        "permission denied",
                    );
                    return;
                }

                let entities = self
                    .world
                    .resource::<session::SessionRegistry>()
                    .entities_by_uid(uid);
                for entity in entities {
                    self.kill_session(entity);
                }

                object.send_ok();
            }
        }
    }
}

pub fn is_lingering(world: &world::World, uid: u32) -> bool {
    world
        .resource::<UserRegistry>()
        .by_uid
        .get(&uid)
        .copied()
        .and_then(|entity| world.get::<Lingering>(entity))
        .is_some()
}
