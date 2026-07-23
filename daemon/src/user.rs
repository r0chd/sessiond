use crate::{ecs, session, session_core_user, session_core_v1, user};
use bevy_ecs::prelude::*;
use bevy_ecs::{entity, world};
use std::{fs, io};

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
    let mut users = world.try_query_filtered::<(Entity, &ecs::UserObjects), With<ecs::User>>()?;
    users.iter(world).find_map(|(entity, objects)| {
        objects
            .0
            .iter()
            .any(|candidate| candidate == object)
            .then_some(entity)
    })
}

pub fn create(world: &mut world::World, uid: u32, lingering: bool) -> Option<entity::Entity> {
    let user = uzers::get_user_by_uid(uid)?;
    let state = compute_state(world, uid, lingering);
    Some(ecs::insert_user(
        world,
        uid,
        user.name().to_string_lossy().into_owned(),
        lingering,
        state,
    ))
}

fn compute_state(world: &world::World, uid: u32, lingering: bool) -> session_core_v1::UserState {
    let states = session::entities_by_uid(world, uid)
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

pub fn sync_all(world: &mut world::World) {
    let Some(mut users) = world.try_query_filtered::<Entity, With<ecs::User>>() else {
        return;
    };
    let users = users.iter(world).collect::<Vec<_>>();

    for entity in users {
        sync(world, entity);
    }
}

fn sync(world: &mut world::World, entity: entity::Entity) {
    let Some(uid) = world.get::<ecs::Uid>(entity).map(|uid| uid.0) else {
        return;
    };
    let lingering = world.get::<ecs::Lingering>(entity).is_some();
    let state = compute_state(world, uid, lingering);

    let changed = world
        .get::<ecs::UserState>(entity)
        .is_none_or(|current| current.0 != state);
    if !changed {
        return;
    }

    if let Some(mut component) = world.get_mut::<ecs::UserState>(entity) {
        component.0 = state;
    }
    if let Some(objects) = world.get::<ecs::UserObjects>(entity) {
        for object in &objects.0 {
            object.send_state(state);
        }
    }
}

pub fn add_object(
    world: &mut world::World,
    entity: entity::Entity,
    object: session_core_user::SessionCoreUser,
    management_access: &crate::Access,
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
    if let Some(client) = object.client() {
        let uid = world.get::<ecs::Uid>(entity).map_or(0, |uid| uid.0);
        let can_modify =
            management_access.allows(client.creds()) || uid == client.creds().uid.as_raw();
        object.send_can_modify(u32::from(can_modify));
    }
    object.send_done();

    if let Some(mut objects) = world.get_mut::<ecs::UserObjects>(entity) {
        objects.0.push(object);
    }
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
        let Some(client) = object.client() else {
            return;
        };

        match event {
            session_core_user::Event::Destroy => {
                if let Some(mut objects) = self.world.get_mut::<ecs::UserObjects>(entity) {
                    objects.0.retain(|candidate| candidate != object);
                }
            }
            session_core_user::Event::Terminate => {
                let uid = self.world.get::<ecs::Uid>(entity).map_or(0, |uid| uid.0);
                if !self.management_access.allows(client.creds())
                    && uid != client.creds().uid.as_raw()
                {
                    object.send_error(
                        session_core_v1::ErrorCode::PermissionDenied,
                        "permission denied",
                    );
                    return;
                }

                let entities = session::entities_by_uid(&self.world, uid).collect::<Vec<_>>();
                for entity in entities {
                    self.terminate_session(entity);
                }

                user::sync_all(&mut self.world);
                object.send_ok();
            }
            session_core_user::Event::Kill => {
                let uid = self.world.get::<ecs::Uid>(entity).map_or(0, |uid| uid.0);
                if !self.management_access.allows(client.creds())
                    && uid != client.creds().uid.as_raw()
                {
                    object.send_error(
                        session_core_v1::ErrorCode::PermissionDenied,
                        "permission denied",
                    );
                    return;
                }

                let entities = session::entities_by_uid(&self.world, uid).collect::<Vec<_>>();
                for entity in entities {
                    self.kill_session(entity);
                }

                user::sync_all(&mut self.world);
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
