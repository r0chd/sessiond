use crate::{ecs, session, session_core_user, session_core_v1};
use bevy_ecs::{entity, world};
use std::{fs, io, path};

pub fn entity_by_uid(
    world: &world::World,
    entities: &[entity::Entity],
    uid: u32,
) -> Option<entity::Entity> {
    entities.iter().copied().find(|entity| {
        world
            .get::<ecs::Uid>(*entity)
            .is_some_and(|entity_uid| entity_uid.0 == uid)
    })
}

pub fn entity_by_object(
    world: &world::World,
    entities: &[entity::Entity],
    object: &session_core_user::SessionCoreUser,
) -> Option<entity::Entity> {
    entities.iter().copied().find(|entity| {
        world
            .get::<ecs::UserObjects>(*entity)
            .is_some_and(|objects| objects.0.iter().any(|candidate| candidate == object))
    })
}

pub fn get_or_create(
    world: &mut world::World,
    entities: &mut Vec<entity::Entity>,
    session_entities: &[entity::Entity],
    uid: u32,
) -> Option<entity::Entity> {
    if let Some(entity) = entity_by_uid(world, entities, uid) {
        return Some(entity);
    }

    let user = users::get_user_by_uid(uid)?;
    let lingering = linger_path(uid).exists();
    let state = compute_state(world, session_entities, uid, lingering);
    let entity = ecs::insert_user(
        world,
        uid,
        user.name().to_string_lossy().into_owned(),
        lingering,
        state,
    );
    entities.push(entity);
    Some(entity)
}

fn compute_state(
    world: &world::World,
    session_entities: &[entity::Entity],
    uid: u32,
    lingering: bool,
) -> session_core_v1::UserState {
    let states = session::entities(world, session_entities)
        .filter(|entity| {
            world
                .get::<ecs::Uid>(*entity)
                .is_some_and(|entity_uid| entity_uid.0 == uid)
        })
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

pub fn sync_all(
    world: &mut world::World,
    user_entities: &[entity::Entity],
    session_entities: &[entity::Entity],
) {
    for entity in user_entities.iter().copied() {
        sync(world, session_entities, entity);
    }
}

fn sync(world: &mut world::World, session_entities: &[entity::Entity], entity: entity::Entity) {
    let Some(uid) = world.get::<ecs::Uid>(entity).map(|uid| uid.0) else {
        return;
    };
    let lingering = world
        .get::<ecs::Lingering>(entity)
        .is_some_and(|lingering| lingering.0);
    let state = compute_state(world, session_entities, uid, lingering);

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

pub fn add_user_object(
    world: &mut world::World,
    entity: entity::Entity,
    object: session_core_user::SessionCoreUser,
) {
    if let Some(uid) = world.get::<ecs::Uid>(entity) {
        object.send_uid(uid.0);
    }
    if let Some(username) = world.get::<ecs::Username>(entity) {
        object.send_username(&username.0);
    }
    if let Some(lingering) = world.get::<ecs::Lingering>(entity) {
        object.send_lingering(u32::from(lingering.0));
    }
    if let Some(state) = world.get::<ecs::UserState>(entity) {
        object.send_state(state.0);
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
        let Some(entity) = entity_by_object(&self.world, &self.user_entities, object) else {
            return;
        };

        if matches!(event, session_core_user::Event::Destroy) {
            if let Some(mut objects) = self.world.get_mut::<ecs::UserObjects>(entity) {
                objects.0.retain(|candidate| candidate != object);
            }
            return;
        }

        let Some(client) = object.client() else {
            return;
        };
        let uid = self.world.get::<ecs::Uid>(entity).map_or(0, |uid| uid.0);
        if !client.creds().uid.is_root() && client.creds().uid.as_raw() != uid {
            object.send_error(
                session_core_v1::ErrorCode::PermissionDenied,
                "permission denied",
            );
            return;
        }

        if let session_core_user::Event::SetLingering { lingering } = event {
            if let Err(err) = set_lingering(uid, lingering != 0) {
                object.send_error(
                    session_core_v1::ErrorCode::Io,
                    format!("failed to set lingering: {err}"),
                );
                return;
            }

            log::info!(uid, lingering; "lingering updated");
            if let Some(mut component) = self.world.get_mut::<ecs::Lingering>(entity) {
                component.0 = lingering != 0;
            }
            if let Some(objects) = self.world.get::<ecs::UserObjects>(entity) {
                for object in &objects.0 {
                    object.send_lingering(lingering);
                    object.send_ok();
                }
            }
        }
    }
}

fn linger_path(uid: u32) -> path::PathBuf {
    path::Path::new("/var/lib/sessiond").join(uid.to_string())
}

fn set_lingering(uid: u32, lingering: bool) -> io::Result<()> {
    if lingering {
        fs::File::create(linger_path(uid))?;
        return Ok(());
    }

    match fs::remove_file(linger_path(uid)) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err),
    }
}
