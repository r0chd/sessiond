use crate::session_core_v1::{session_core_seat_client, session_core_seat_device};
use crate::{device, ecs, reconcile, seat, seat_v1, session, session_core_v1};
use bevy_ecs::prelude::*;
use bevy_ecs::{entity, world};

pub fn spawn(
    world: &mut world::World,
    object: seat_v1::seat::Seat,
    pid: u32,
    session: session::SessionId,
) -> entity::Entity {
    world
        .spawn(ecs::ClientBundle {
            marker: ecs::Client,
            seat_object: ecs::SeatObject(object),
            pid: ecs::ClientPid(pid),
            state: ecs::SeatState(seat::SeatState::New),
            session: ecs::ClientSession(session),
        })
        .id()
}

pub const fn state_event(state: seat::SeatState) -> session_core_v1::SeatClientState {
    match state {
        seat::SeatState::New => session_core_v1::SeatClientState::New,
        seat::SeatState::Enabled => session_core_v1::SeatClientState::Enabled,
        seat::SeatState::PendingDisabled => session_core_v1::SeatClientState::PendingDisabled,
        seat::SeatState::Disabled => session_core_v1::SeatClientState::Disabled,
    }
}

pub fn is_active(
    world: &world::World,
    seat_entity: entity::Entity,
    client_entity: entity::Entity,
) -> bool {
    world
        .get::<ecs::ActiveClients>(seat_entity)
        .is_some_and(|active_client| active_client.0.contains(&client_entity))
}

pub fn session_id(
    world: &world::World,
    client_entity: entity::Entity,
) -> anyhow::Result<session::SessionId> {
    world
        .get::<ecs::SessionId>(client_entity)
        .map(|session| session.0)
        .ok_or_else(|| anyhow::anyhow!("client has no session id"))
}

pub fn add_object(
    world: &mut world::World,
    seat_entity: entity::Entity,
    client_entity: entity::Entity,
    object: session_core_seat_client::SessionCoreSeatClient,
) {
    if let Some(session) = world
        .get::<ecs::ClientSession>(client_entity)
        .map(|session| session.0)
        .or_else(|| {
            world
                .get::<ecs::SessionId>(client_entity)
                .map(|session| session.0)
        })
    {
        object.send_session(session.as_raw());
    }
    if let Some(state) = world.get::<ecs::SeatState>(client_entity) {
        object.send_state(state_event(state.0));
    }
    object.send_active(u32::from(is_active(world, seat_entity, client_entity)));
    object.send_next(u32::from(
        world
            .get::<ecs::NextClients>(seat_entity)
            .is_some_and(|next| next.0.contains(&client_entity)),
    ));
    if let Some(pid) = world.get::<ecs::ClientPid>(client_entity) {
        object.send_pid(pid.0);
    }
    for device_entity in device::devices(world, client_entity) {
        if let Some(objects) = world
            .resource::<ecs::ProtocolObjects>()
            .device_objects(device_entity)
        {
            for device_object in objects {
                object.send_device_added(device_object.id);
            }
        }
    }
    object.send_done();

    world
        .resource_mut::<ecs::ProtocolObjects>()
        .add_client_object(client_entity, object);
}

pub fn entity_by_seat_object(
    world: &world::World,
    object: &seat_v1::seat::Seat,
) -> Option<entity::Entity> {
    let mut clients =
        world.try_query_filtered::<(Entity, &ecs::SeatObject), With<ecs::Client>>()?;
    clients
        .iter(world)
        .find_map(|(entity, seat_object)| (seat_object.0 == *object).then_some(entity))
}

pub fn entity_by_core_object(
    world: &world::World,
    object: &session_core_seat_client::SessionCoreSeatClient,
) -> Option<entity::Entity> {
    world
        .resource::<ecs::ProtocolObjects>()
        .client_entity_by_object(object)
}

pub fn entity_by_pid(world: &world::World, pid: u32) -> Option<entity::Entity> {
    let mut clients = world.try_query_filtered::<(Entity, &ecs::ClientPid), With<ecs::Client>>()?;
    clients
        .iter(world)
        .find_map(|(entity, client_pid)| (client_pid.0 == pid).then_some(entity))
}

pub fn entity_by_device_object(
    world: &world::World,
    object: &seat_v1::device::Device,
) -> Option<entity::Entity> {
    let mut devices =
        world.try_query_filtered::<(&ecs::OwnerClient, Entity), With<ecs::Device>>()?;
    devices.iter(world).find_map(|(client, device_entity)| {
        device::contains_object(world, device_entity, object).then_some(client.0)
    })
}

pub fn entity_by_core_device_object(
    world: &world::World,
    object: &session_core_seat_device::SessionCoreSeatDevice,
) -> Option<entity::Entity> {
    let mut devices =
        world.try_query_filtered::<(&ecs::OwnerClient, Entity), With<ecs::Device>>()?;
    devices.iter(world).find_map(|(client, device_entity)| {
        world
            .resource::<ecs::ProtocolObjects>()
            .seat_device_objects(device_entity)
            .is_some_and(|objects| objects.iter().any(|candidate| candidate.object == *object))
            .then_some(client.0)
    })
}

impl hyprwire::Dispatch<session_core_seat_client::SessionCoreSeatClient> for crate::Sessiond {
    fn event(
        &mut self,
        object: &session_core_seat_client::SessionCoreSeatClient,
        event: <session_core_seat_client::SessionCoreSeatClient as hyprwire::Object>::Event<'_>,
    ) {
        match event {
            session_core_seat_client::Event::GetDevice {
                session_core_seat_device,
                device_id,
            } => {
                let Some(client_entity) = entity_by_core_object(&self.world, object) else {
                    session_core_seat_device.send_destroyed();
                    return;
                };
                let Some(device_entity) =
                    device::entity_by_id(&self.world, client_entity, device_id)
                else {
                    session_core_seat_device.send_destroyed();
                    return;
                };

                device::add_core_object(
                    &mut self.world,
                    device_entity,
                    device_id,
                    session_core_seat_device,
                );
            }
            session_core_seat_client::Event::Destroy => {
                if let Some(client_entity) = entity_by_core_object(&self.world, object) {
                    self.world
                        .resource_mut::<ecs::ProtocolObjects>()
                        .remove_client_object(client_entity, object);
                }
            }
        }
    }
}

pub fn set_state(world: &mut world::World, client_entity: entity::Entity, state: seat::SeatState) {
    world
        .entity_mut(client_entity)
        .insert(ecs::SeatState(state));
}

pub fn deactivate_devices(world: &mut world::World, client_entity: entity::Entity) {
    let devices = device::devices(world, client_entity);

    for device_entity in devices {
        if let Err(err) = device::deactivate(world, device_entity) {
            let path = device::path_display(world, device_entity);
            log::error!("Could not deactivate {path}: {err}");
            continue;
        }

        match device::kind(world, device_entity) {
            Ok(device::DeviceKind::Drm | device::DeviceKind::Wscons) => {}
            Ok(device::DeviceKind::Evdev | device::DeviceKind::Hidraw) => {
                reconcile::device_removed(world, client_entity, device_entity);
                device::remove_protocol_objects(world, device_entity);
                world.despawn(device_entity);
            }
            Err(err) => {
                log::error!("Could not determine device kind: {err}");
            }
        }
    }
}

pub fn add(
    world: &mut world::World,
    seat_entity: entity::Entity,
    client_entity: entity::Entity,
) -> anyhow::Result<()> {
    if world.get::<ecs::SeatName>(client_entity).is_some() {
        log::error!("Could not add client: client is already a member of a seat");
        return Err(anyhow::anyhow!("client is already a member of a seat"));
    }
    if world.get::<ecs::SessionId>(client_entity).is_some() {
        log::error!("Could not add client: client cannot be reused");
        return Err(anyhow::anyhow!("client cannot be reused"));
    }

    if let Some(pid) = world.get::<ecs::ClientPid>(client_entity).map(|pid| pid.0)
        && seat::clients(world, seat_entity).into_iter().any(|client| {
            world
                .get::<ecs::ClientPid>(client)
                .is_some_and(|client_pid| client_pid.0 == pid)
        })
    {
        return Err(anyhow::anyhow!("PID is already attached to the seat"));
    }

    let name = seat::name(world, seat_entity)?;
    seat::refresh_current_vt(world, seat_entity);

    let mode = seat::mode(world, seat_entity)?;
    let session_id = match mode {
        ecs::SeatKind::VtBound => {
            let vt = world
                .get::<ecs::CurrentVt>(seat_entity)
                .map(|vt| vt.0)
                .ok_or_else(|| anyhow::anyhow!("could not determine VT for client"))?;

            session::SessionId::from_raw(vt.get())
        }
        ecs::SeatKind::NonVtBound => {
            let mut next_session = 1;
            while seat::clients(world, seat_entity).into_iter().any(|client| {
                world
                    .get::<ecs::SessionId>(client)
                    .is_some_and(|session| session.0.as_raw() == next_session)
            }) {
                next_session += 1;
            }

            session::SessionId::from_raw(next_session)
        }
    };

    if mode == ecs::SeatKind::VtBound
        && !seat::clients(world, seat_entity).into_iter().any(|client| {
            world
                .get::<ecs::SessionId>(client)
                .is_some_and(|session| session.0 == session_id)
        })
    {
        seat::vt_open(session_id.as_raw())?;
    }

    world.entity_mut(client_entity).insert((
        ecs::SeatName(name.clone()),
        ecs::ClientSeat(seat_entity),
        ecs::SessionId(session_id),
    ));
    set_state(world, client_entity, seat::SeatState::Enabled);
    seat::set_active_client(world, seat_entity, Some(client_entity));
    seat::notify_client_added(world, seat_entity, client_entity);

    if let Some(object) = world.get::<ecs::SeatObject>(client_entity) {
        object.0.send_enabled();
    }
    log::info!(
        session_id = session_id.as_raw(),
        seat = name.as_str(),
        pid = world
            .get::<ecs::ClientPid>(client_entity)
            .map_or(0, |pid| pid.0);
        "added client to seat"
    );

    Ok(())
}

pub fn open(
    world: &mut world::World,
    seat_entity: entity::Entity,
    client_entity: entity::Entity,
) -> anyhow::Result<()> {
    let name = seat::name(world, seat_entity)?;
    let session_id = session_id(world, client_entity)?;
    let state = seat::state(world, client_entity)?;

    if state != seat::SeatState::New && state != seat::SeatState::Disabled {
        log::error!("Could not enable client: client is not new or disabled");
        return Err(anyhow::anyhow!("client is not new or disabled"));
    }

    if seat::is_vt_bound(world, seat_entity) {
        seat::vt_open(session_id.as_raw())?;
    }

    let devices = device::devices(world, client_entity);
    for device_entity in devices {
        if let Err(err) = device::activate(world, device_entity) {
            let path = device::path_display(world, device_entity);
            log::error!("Could not activate {path}: {err}");
        }
    }

    if let Some(object) = world.get::<ecs::SeatObject>(client_entity) {
        object.0.send_enabled();
    }
    set_state(world, client_entity, seat::SeatState::Enabled);
    seat::set_active_client(world, seat_entity, Some(client_entity));
    log::info!("Opened client {} on {}", session_id.as_raw(), name);

    Ok(())
}

pub fn disable(
    world: &mut world::World,
    seat_entity: entity::Entity,
    client_entity: entity::Entity,
) -> anyhow::Result<()> {
    if seat::state(world, client_entity)? != seat::SeatState::Enabled {
        log::error!("Could not disable client: client is not active");
        return Err(anyhow::anyhow!("client is not active"));
    }

    if !is_active(world, seat_entity, client_entity) {
        log::error!("Could not disable client: client is not the active seat client");
        return Err(anyhow::anyhow!("client is not the active seat client"));
    }

    let session_id = session_id(world, client_entity)?;
    let name = seat::name(world, seat_entity)?;

    deactivate_devices(world, client_entity);

    set_state(world, client_entity, seat::SeatState::PendingDisabled);
    if let Some(object) = world.get::<ecs::SeatObject>(client_entity) {
        object.0.send_disabled();
    }
    log::info!("Disabling client {} on {}", session_id.as_raw(), name);

    Ok(())
}

pub fn ack_disable(
    world: &mut world::World,
    seat_entity: entity::Entity,
    client_entity: entity::Entity,
) -> anyhow::Result<()> {
    if seat::state(world, client_entity)? != seat::SeatState::PendingDisabled {
        log::error!("Could not ack disable: client is not pending disable");
        return Err(anyhow::anyhow!("client is not pending disable"));
    }

    let session_id = session_id(world, client_entity)?;
    let name = seat::name(world, seat_entity)?;
    set_state(world, client_entity, seat::SeatState::Disabled);
    log::info!("Disabled client {} on {}", session_id.as_raw(), name);

    if !is_active(world, seat_entity, client_entity) {
        return Ok(());
    }

    if let Some(mut active_clients) = world.get_mut::<ecs::ActiveClients>(seat_entity) {
        active_clients.0.retain(|client| *client != client_entity);
        let no_active_clients = active_clients.0.is_empty();
        if no_active_clients {
            seat::set_active_client(world, seat_entity, None);
            let _ = seat::activate(world, seat_entity);
        }
    }

    Ok(())
}
