#![allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "Bevy system parameters are passed by value and often have complex Query types"
)]

use crate::{client, ecs};
use bevy_ecs::prelude::*;
use std::mem;

pub fn notify_schedule() -> Schedule {
    let mut schedule = Schedule::new(ecs::Notify);
    schedule.add_systems(
        (
            notify_client_states,
            notify_session_active_states,
            notify_user_states,
            notify_seat_clients,
            notify_device_active_states,
            notify_pending_device_active_states,
        )
            .before(send_done),
    );
    schedule.add_systems(send_done);
    schedule
}

fn notify_client_states(
    objects: Res<ecs::ProtocolObjects>,
    mut batch: ResMut<ecs::NotifyBatch>,
    clients: Query<(Entity, &ecs::SeatState), (With<ecs::Client>, Changed<ecs::SeatState>)>,
) {
    for (entity, state) in &clients {
        if let Some(objects) = objects.client_objects(entity) {
            for object in objects {
                object.send_state(client::state_event(state.0));
            }
            batch.client(entity);
        }
    }
}

fn notify_session_active_states(
    objects: Res<ecs::ProtocolObjects>,
    mut batch: ResMut<ecs::NotifyBatch>,
    sessions: Query<(Entity, &ecs::ActiveState), (With<ecs::Session>, Changed<ecs::ActiveState>)>,
) {
    for (entity, state) in &sessions {
        if let Some(objects) = objects.session_objects(entity) {
            for object in objects {
                object.send_active(state.0);
            }
            batch.session(entity);
        }
    }
}

fn notify_user_states(
    objects: Res<ecs::ProtocolObjects>,
    mut batch: ResMut<ecs::NotifyBatch>,
    users: Query<(Entity, &ecs::UserState), (With<ecs::User>, Changed<ecs::UserState>)>,
) {
    for (entity, state) in &users {
        if let Some(objects) = objects.user_objects(entity) {
            for object in objects {
                object.send_state(state.0);
            }
            batch.user(entity);
        }
    }
}

fn notify_seat_clients(
    objects: Res<ecs::ProtocolObjects>,
    mut batch: ResMut<ecs::NotifyBatch>,
    mut state: ResMut<ecs::SeatNotifyState>,
    seats: Query<
        (
            Entity,
            Option<&ecs::ActiveClients>,
            Option<&ecs::NextClients>,
        ),
        With<ecs::Seat>,
    >,
) {
    for (seat_entity, active_clients, next_clients) in &seats {
        let active_clients = active_clients
            .map(|clients| clients.0.clone())
            .filter(|clients| !clients.is_empty())
            .unwrap_or_default();
        let previous_active = state
            .active_clients
            .insert(seat_entity, active_clients.clone())
            .unwrap_or_default();
        for client in previous_active
            .iter()
            .filter(|client| !active_clients.contains(client))
        {
            send_client_active(&objects, &mut batch, *client, false);
        }
        for client in active_clients
            .iter()
            .filter(|client| !previous_active.contains(client))
        {
            send_client_active(&objects, &mut batch, *client, true);
        }

        let next_clients = next_clients
            .map(|clients| clients.0.clone())
            .filter(|clients| !clients.is_empty())
            .unwrap_or_default();
        let previous_next = state
            .next_clients
            .insert(seat_entity, next_clients.clone())
            .unwrap_or_default();
        for client in previous_next
            .iter()
            .filter(|client| !next_clients.contains(client))
        {
            send_client_next(&objects, &mut batch, *client, false);
        }
        for client in next_clients
            .iter()
            .filter(|client| !previous_next.contains(client))
        {
            send_client_next(&objects, &mut batch, *client, true);
        }
    }
}

fn send_client_active(
    objects: &ecs::ProtocolObjects,
    batch: &mut ecs::NotifyBatch,
    entity: Entity,
    active: bool,
) {
    if let Some(objects) = objects.client_objects(entity) {
        for object in objects {
            object.send_active(u32::from(active));
        }
        batch.client(entity);
    }
}

fn send_client_next(
    objects: &ecs::ProtocolObjects,
    batch: &mut ecs::NotifyBatch,
    entity: Entity,
    next: bool,
) {
    if let Some(objects) = objects.client_objects(entity) {
        for object in objects {
            object.send_next(u32::from(next));
        }
        batch.client(entity);
    }
}

fn notify_device_active_states(
    objects: Res<ecs::ProtocolObjects>,
    mut batch: ResMut<ecs::NotifyBatch>,
    devices: Query<Entity, (With<ecs::Device>, Added<ecs::DeviceActive>)>,
) {
    for entity in &devices {
        if let Some(objects) = objects.seat_device_objects(entity) {
            for object in objects {
                object.object.send_active(1);
            }
            batch.device(entity);
        }
    }
}

fn notify_pending_device_active_states(
    objects: Res<ecs::ProtocolObjects>,
    mut batch: ResMut<ecs::NotifyBatch>,
    mut pending: ResMut<ecs::PendingDeviceActiveNotifications>,
) {
    let pending = mem::take(&mut pending.0);
    for (entity, active) in pending {
        if let Some(objects) = objects.seat_device_objects(entity) {
            for object in objects {
                object.object.send_active(u32::from(active));
            }
            batch.device(entity);
        }
    }
}

fn send_done(objects: Res<ecs::ProtocolObjects>, mut batch: ResMut<ecs::NotifyBatch>) {
    for entity in mem::take(&mut batch.sessions) {
        if let Some(objects) = objects.session_objects(entity) {
            for object in objects {
                object.send_done();
            }
        }
    }

    for entity in mem::take(&mut batch.users) {
        if let Some(objects) = objects.user_objects(entity) {
            for object in objects {
                object.send_done();
            }
        }
    }

    for entity in mem::take(&mut batch.clients) {
        if let Some(objects) = objects.client_objects(entity) {
            for object in objects {
                object.send_done();
            }
        }
    }

    for entity in mem::take(&mut batch.devices) {
        if let Some(objects) = objects.seat_device_objects(entity) {
            for object in objects {
                object.object.send_done();
            }
        }
    }
}

pub fn client_device_added(world: &World, client_entity: Entity, device_id: u32) {
    if let Some(objects) = world
        .resource::<ecs::ProtocolObjects>()
        .client_objects(client_entity)
    {
        for object in objects {
            object.send_device_added(device_id);
        }
    }
}

pub fn client_device_removed(world: &World, client_entity: Entity, device_id: u32) {
    if let Some(objects) = world
        .resource::<ecs::ProtocolObjects>()
        .client_objects(client_entity)
    {
        for object in objects {
            object.send_device_removed(device_id);
        }
    }
}

pub fn client_destroyed(world: &World, client_entity: Entity) {
    if let Some(objects) = world
        .resource::<ecs::ProtocolObjects>()
        .client_objects(client_entity)
    {
        for object in objects {
            object.send_destroyed();
        }
    }
}

pub fn device_removed(world: &World, client_entity: Entity, device_entity: Entity) {
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

pub fn device_destroyed(world: &World, device_entity: Entity) {
    if let Some(objects) = world
        .resource::<ecs::ProtocolObjects>()
        .seat_device_objects(device_entity)
    {
        for object in objects {
            object.object.send_destroyed();
        }
    }
}

pub fn device_id_destroyed(world: &World, device_entity: Entity, device_id: u32) {
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
