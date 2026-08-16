use crate::{ecs, session, session_core_user, session_core_v1};
use bevy_ecs::prelude::*;
use bevy_ecs::{entity, world};
use std::{fs, io};

pub fn spawn(
    world: &mut world::World,
    uid: u32,
    username: String,
    lingering: bool,
    state: session_core_v1::UserState,
) -> entity::Entity {
    let mut cmd = world.spawn(ecs::UserBundle {
        marker: ecs::User,
        uid: ecs::Uid(uid),
        username: ecs::Username(username),
        state: ecs::UserState(state),
    });

    if lingering {
        cmd.insert(ecs::Lingering);
    }

    cmd.id()
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

pub fn entity_by_uid(world: &world::World, uid: u32) -> Option<entity::Entity> {
    let mut users = world.try_query_filtered::<(Entity, &ecs::Uid), With<ecs::User>>()?;
    users
        .iter(world)
        .find_map(|(entity, entity_uid)| (entity_uid.0 == uid).then_some(entity))
}

pub fn entity_by_object(
    world: &world::World,
    object: &session_core_user::SessionCoreUser,
) -> Option<entity::Entity> {
    world
        .resource::<ecs::ProtocolObjects>()
        .user_entity_by_object(object)
}

pub fn create(world: &mut world::World, uid: u32, lingering: bool) -> Option<entity::Entity> {
    let user = uzers::get_user_by_uid(uid)?;
    let state = compute_state(world, uid, lingering);
    Some(spawn(
        world,
        uid,
        user.name().to_string_lossy().into_owned(),
        lingering,
        state,
    ))
}

fn compute_state(world: &world::World, uid: u32, lingering: bool) -> session_core_v1::UserState {
    let states = session::entities_by_uid(world, uid)
        .into_iter()
        .filter_map(|entity| world.get::<ecs::ActiveState>(entity).map(|state| state.0))
        .collect::<Vec<_>>();

    if states.contains(&session_core_v1::ActiveState::Active) {
        return session_core_v1::UserState::Active;
    }

    if !states.is_empty() {
        return if states
            .iter()
            .all(|state| *state == session_core_v1::ActiveState::Closing)
        {
            session_core_v1::UserState::Closing
        } else {
            session_core_v1::UserState::Online
        };
    }

    if lingering {
        session_core_v1::UserState::Lingering
    } else {
        session_core_v1::UserState::Offline
    }
}

pub fn reconcile_user_states(
    users: Query<(&ecs::Uid, Option<&ecs::Lingering>, &mut ecs::UserState), With<ecs::User>>,
    sessions: Query<(&ecs::Uid, &ecs::ActiveState), With<ecs::Session>>,
) {
    for (uid, lingering, mut user_state) in users {
        let states: Vec<_> = sessions
            .iter()
            .filter(|(session_uid, _)| session_uid.0 == uid.0)
            .map(|(_, state)| state.0)
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

        if user_state.0 != desired {
            user_state.0 = desired;
        }
    }
}

pub fn add_object(
    world: &mut world::World,
    entity: entity::Entity,
    object: session_core_user::SessionCoreUser,
    management_access: crate::Access,
) {
    if let Some(uid) = world.get::<ecs::Uid>(entity) {
        object.send_uid(uid.0);
    }
    if let Some(username) = world.get::<ecs::Username>(entity) {
        object.send_username(&username.0);
    }
    if world.get::<ecs::Lingering>(entity).is_some() {
        object.send_lingering(1);
    } else {
        object.send_lingering(0);
    }
    if let Some(state) = world.get::<ecs::UserState>(entity) {
        object.send_state(state.0);
    }
    let client = object.client();
    let uid = world.get::<ecs::Uid>(entity).map_or(0, |uid| uid.0);
    let can_modify = management_access.allows(client.creds()) || uid == client.creds().uid.as_raw();
    object.send_can_modify(u32::from(can_modify));
    object.send_done();

    world
        .resource_mut::<ecs::ProtocolObjects>()
        .add_user_object(entity, object);
}

impl hyprwire::Dispatch<session_core_user::SessionCoreUser> for crate::Sessiond {
    fn event(
        &mut self,
        object: &session_core_user::SessionCoreUser,
        event: <session_core_user::SessionCoreUser as hyprwire::Object>::Event<'_>,
    ) {
        let Some(entity) = entity_by_object(&self.world, object) else {
            return;
        };
        let client = object.client();

        match event {
            session_core_user::Event::Destroy => {
                self.world
                    .resource_mut::<ecs::ProtocolObjects>()
                    .remove_user_object(entity, object);
            }
            session_core_user::Event::Terminate => {
                let uid = self.world.get::<ecs::Uid>(entity).map_or(0, |uid| uid.0);
                if !self
                    .world
                    .resource::<ecs::AccessRules>()
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

                let entities = session::entities_by_uid(&self.world, uid);
                for entity in entities {
                    self.terminate_session(entity);
                }

                object.send_ok();
            }
            session_core_user::Event::Kill => {
                let uid = self.world.get::<ecs::Uid>(entity).map_or(0, |uid| uid.0);
                if !self
                    .world
                    .resource::<ecs::AccessRules>()
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

                let entities = session::entities_by_uid(&self.world, uid);
                for entity in entities {
                    self.kill_session(entity);
                }

                object.send_ok();
            }
        }
    }
}

pub fn is_lingering(world: &world::World, uid: u32) -> bool {
    entity_by_uid(world, uid)
        .and_then(|entity| world.get::<ecs::Lingering>(entity))
        .is_some()
}
