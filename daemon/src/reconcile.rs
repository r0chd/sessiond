use crate::{client, ecs};
use bevy_ecs::{entity, world};

pub fn client_state(world: &world::World, client_entity: entity::Entity) {
    let Some(state) = world
        .get::<ecs::SeatState>(client_entity)
        .map(|state| state.0)
    else {
        return;
    };

    if let Some(objects) = world
        .resource::<ecs::ProtocolObjects>()
        .client_objects(client_entity)
    {
        for object in objects {
            object.send_state(client::state_event(state));
        }
    }
}

pub fn client_active(
    world: &world::World,
    seat_entity: entity::Entity,
    client_entity: entity::Entity,
) {
    let active = u32::from(client::is_active(world, seat_entity, client_entity));
    if let Some(objects) = world
        .resource::<ecs::ProtocolObjects>()
        .client_objects(client_entity)
    {
        for object in objects {
            object.send_active(active);
        }
    }
}

pub fn client_next(world: &world::World, client_entity: entity::Entity, next: bool) {
    if let Some(objects) = world
        .resource::<ecs::ProtocolObjects>()
        .client_objects(client_entity)
    {
        for object in objects {
            object.send_next(u32::from(next));
        }
    }
}

pub fn client_device_added(world: &world::World, client_entity: entity::Entity, device_id: u32) {
    if let Some(objects) = world
        .resource::<ecs::ProtocolObjects>()
        .client_objects(client_entity)
    {
        for object in objects {
            object.send_device_added(device_id);
        }
    }
}

pub fn client_device_removed(world: &world::World, client_entity: entity::Entity, device_id: u32) {
    if let Some(objects) = world
        .resource::<ecs::ProtocolObjects>()
        .client_objects(client_entity)
    {
        for object in objects {
            object.send_device_removed(device_id);
        }
    }
}

pub fn client_destroyed(world: &world::World, client_entity: entity::Entity) {
    if let Some(objects) = world
        .resource::<ecs::ProtocolObjects>()
        .client_objects(client_entity)
    {
        for object in objects {
            object.send_destroyed();
        }
    }
}

pub fn device_active(world: &world::World, device_entity: entity::Entity, active: bool) {
    if let Some(objects) = world
        .resource::<ecs::ProtocolObjects>()
        .seat_device_objects(device_entity)
    {
        for object in objects {
            object.object.send_active(u32::from(active));
        }
    }
}

pub fn device_removed(
    world: &world::World,
    client_entity: entity::Entity,
    device_entity: entity::Entity,
) {
    if let Some(objects) = world
        .resource::<ecs::ProtocolObjects>()
        .device_objects(device_entity)
    {
        for device_object in objects {
            client_device_removed(world, client_entity, device_object.id);
        }
    }
    device_destroyed(world, device_entity);
}

pub fn device_destroyed(world: &world::World, device_entity: entity::Entity) {
    if let Some(objects) = world
        .resource::<ecs::ProtocolObjects>()
        .seat_device_objects(device_entity)
    {
        for object in objects {
            object.object.send_destroyed();
        }
    }
}

pub fn device_id_destroyed(world: &world::World, device_entity: entity::Entity, device_id: u32) {
    if let Some(objects) = world
        .resource::<ecs::ProtocolObjects>()
        .seat_device_objects(device_entity)
    {
        for object in objects {
            if object.id == device_id {
                object.object.send_destroyed();
            }
        }
    }
}

pub fn session_active(world: &world::World, session_entity: entity::Entity) {
    let Some(state) = world
        .get::<ecs::ActiveState>(session_entity)
        .map(|state| state.0)
    else {
        return;
    };

    if let Some(objects) = world
        .resource::<ecs::ProtocolObjects>()
        .session_objects(session_entity)
    {
        for object in objects {
            object.send_active(state);
        }
    }
}

pub fn user_state(world: &world::World, user_entity: entity::Entity) {
    let Some(state) = world
        .get::<ecs::UserState>(user_entity)
        .map(|state| state.0)
    else {
        return;
    };

    if let Some(objects) = world
        .resource::<ecs::ProtocolObjects>()
        .user_objects(user_entity)
    {
        for object in objects {
            object.send_state(state);
        }
    }
}
