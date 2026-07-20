use crate::session_core_v1::{session_core_seat_client, session_core_seat_device};
use crate::{cgroup, client, device, ecs, seat, seat_v1, session, session_core_v1};
use bevy_ecs::prelude::*;
use bevy_ecs::{entity, world};

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
        .get::<ecs::ActiveClient>(seat_entity)
        .is_some_and(|active_client| active_client.0 == client_entity)
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
        .and_then(|session| session.0)
        .or_else(|| {
            world
                .get::<ecs::SessionId>(client_entity)
                .map(|session| session.0)
        })
    {
        object.send_session(session.as_raw());
    }
    if let Some(state) = world.get::<ecs::SeatState>(client_entity) {
        object.send_state(client::state_event(state.0));
    }
    object.send_active(u32::from(is_active(world, seat_entity, client_entity)));
    object.send_next(u32::from(
        world
            .get::<ecs::NextClient>(seat_entity)
            .is_some_and(|next| next.0 == client_entity),
    ));
    if let Some(pid) = world.get::<ecs::ClientPid>(client_entity) {
        object.send_pid(pid.0);
    }
    if let Some(devices) = world.get::<ecs::Devices>(client_entity) {
        for device_entity in &devices.0 {
            if let Some(objects) = world.get::<ecs::DeviceObjects>(*device_entity) {
                for device_object in &objects.0 {
                    object.send_device_added(device_object.id);
                }
            }
        }
    }
    object.send_done();

    if let Some(mut objects) = world.get_mut::<ecs::ClientObjects>(client_entity) {
        objects.0.push(object);
    }
}

pub fn entity_by_seat_object(
    world: &world::World,
    object: &seat_v1::seat::Seat,
) -> Option<entity::Entity> {
    let mut clients = world.try_query_filtered::<(Entity, &ecs::SeatObject), With<ecs::Client>>()?;
    clients
        .iter(world)
        .find_map(|(entity, seat_object)| (seat_object.0 == *object).then_some(entity))
}

pub fn entity_by_core_object(
    world: &world::World,
    object: &session_core_seat_client::SessionCoreSeatClient,
) -> Option<entity::Entity> {
    let mut clients =
        world.try_query_filtered::<(Entity, &ecs::ClientObjects), With<ecs::Client>>()?;
    clients.iter(world).find_map(|(entity, objects)| {
        objects
            .0
            .iter()
            .any(|candidate| candidate == object)
            .then_some(entity)
    })
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
    let mut clients = world.try_query_filtered::<(Entity, &ecs::Devices), With<ecs::Client>>()?;
    clients.iter(world).find_map(|(entity, devices)| {
        devices
            .0
            .iter()
            .any(|device_entity| device::contains_object(world, *device_entity, object))
            .then_some(entity)
    })
}

pub fn entity_by_core_device_object(
    world: &world::World,
    object: &session_core_seat_device::SessionCoreSeatDevice,
) -> Option<entity::Entity> {
    let mut clients = world.try_query_filtered::<(Entity, &ecs::Devices), With<ecs::Client>>()?;
    clients.iter(world).find_map(|(entity, devices)| {
        devices
            .0
            .iter()
            .any(|device_entity| {
                world
                    .get::<ecs::SeatDeviceObjects>(*device_entity)
                    .is_some_and(|objects| {
                        objects
                            .0
                            .iter()
                            .any(|candidate| candidate.object == *object)
                    })
            })
            .then_some(entity)
    })
}

impl<C: cgroup::CgroupManager + 'static>
    hyprwire::Dispatch<session_core_seat_client::SessionCoreSeatClient> for crate::Sessiond<C>
{
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
                let Some(client_entity) =
                    client::entity_by_core_object(&self.world, object)
                else {
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
                if let Some(client_entity) =
                    client::entity_by_core_object(&self.world, object)
                    && let Some(mut objects) =
                        self.world.get_mut::<ecs::ClientObjects>(client_entity)
                {
                    objects.0.retain(|candidate| candidate != object);
                }
            }
        }
    }
}

pub fn notify_state(world: &world::World, client_entity: entity::Entity) {
    let Some(state) = world
        .get::<ecs::SeatState>(client_entity)
        .map(|state| state.0)
    else {
        return;
    };
    if let Some(objects) = world.get::<ecs::ClientObjects>(client_entity) {
        for object in &objects.0 {
            object.send_state(client::state_event(state));
        }
    }
}

pub fn notify_active(
    world: &world::World,
    seat_entity: entity::Entity,
    client_entity: entity::Entity,
) {
    let active = u32::from(client::is_active(world, seat_entity, client_entity));
    if let Some(objects) = world.get::<ecs::ClientObjects>(client_entity) {
        for object in &objects.0 {
            object.send_active(active);
        }
    }
}

pub fn notify_next(world: &world::World, client_entity: entity::Entity, next: bool) {
    if let Some(objects) = world.get::<ecs::ClientObjects>(client_entity) {
        for object in &objects.0 {
            object.send_next(u32::from(next));
        }
    }
}

pub fn notify_device_added(world: &world::World, client_entity: entity::Entity, device_id: u32) {
    if let Some(objects) = world.get::<ecs::ClientObjects>(client_entity) {
        for object in &objects.0 {
            object.send_device_added(device_id);
        }
    }
}

pub fn notify_device_removed(world: &world::World, client_entity: entity::Entity, device_id: u32) {
    if let Some(objects) = world.get::<ecs::ClientObjects>(client_entity) {
        for object in &objects.0 {
            object.send_device_removed(device_id);
        }
    }
}

pub fn notify_destroyed(world: &world::World, client_entity: entity::Entity) {
    if let Some(objects) = world.get::<ecs::ClientObjects>(client_entity) {
        for object in &objects.0 {
            object.send_destroyed();
        }
    }
}

pub fn deactivate_devices(world: &mut world::World, client_entity: entity::Entity) {
    let Some(devices) = world
        .get::<ecs::Devices>(client_entity)
        .map(|devices| devices.0.clone())
    else {
        return;
    };

    let mut retained_devices = Vec::new();
    for device_entity in devices {
        if let Err(err) = device::deactivate(world, device_entity) {
            let path = device::path_display(world, device_entity);
            log::error!("Could not deactivate {path}: {err}");
            retained_devices.push(device_entity);
            continue;
        }

        match device::kind(world, device_entity) {
            Ok(device::DeviceKind::Drm | device::DeviceKind::Wscons) => {
                retained_devices.push(device_entity);
            }
            Ok(device::DeviceKind::Evdev | device::DeviceKind::Hidraw) => {
                device::notify_removed(world, client_entity, device_entity);
                world.despawn(device_entity);
            }
            Err(err) => {
                log::error!("Could not determine device kind: {err}");
                retained_devices.push(device_entity);
            }
        }
    }

    if let Some(mut devices) = world.get_mut::<ecs::Devices>(client_entity) {
        devices.0 = retained_devices;
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

    let name = seat::name(world, seat_entity)?;
    seat::refresh_current_vts(world, &[seat_entity]);

    let mode = seat::mode(world, seat_entity)?;
    let session_id = match mode {
        ecs::SeatKind::VtBound => {
            if let Some(active_client) = world.get::<ecs::ActiveClient>(seat_entity) {
                let active_state = world
                    .get::<ecs::SeatState>(active_client.0)
                    .map(|state| state.0);
                if active_state != Some(seat::SeatState::PendingDisabled) {
                    log::error!("Could not add client: seat is VT-bound and has an active client");
                    return Err(anyhow::anyhow!("seat is VT-bound and has an active client"));
                }
            }

            let vt = world
                .get::<ecs::CurrentVt>(seat_entity)
                .map(|vt| vt.0)
                .ok_or_else(|| anyhow::anyhow!("could not determine VT for client"))?;
            if vt == 0 {
                log::error!("Could not determine VT for client");
                return Err(anyhow::anyhow!("could not determine VT for client"));
            }

            let already_pending =
                world
                    .get::<ecs::SeatClients>(seat_entity)
                    .is_some_and(|clients| {
                        clients.0.iter().any(|client| {
                            world
                                .get::<ecs::SessionId>(*client)
                                .is_some_and(|session| session.0.as_raw() == vt)
                        })
                    });
            if world.get::<ecs::ActiveClient>(seat_entity).is_some() && already_pending {
                log::error!(
                    "Could not add client: seat is VT-bound and already has pending client"
                );
                return Err(anyhow::anyhow!(
                    "seat is VT-bound and already has pending client"
                ));
            }

            session::SessionId::from_raw(vt)
        }
        ecs::SeatKind::NonVtBound => {
            let mut next_session = 1;
            while world
                .get::<ecs::SeatClients>(seat_entity)
                .is_some_and(|clients| {
                    clients.0.iter().any(|client| {
                        world
                            .get::<ecs::SessionId>(*client)
                            .is_some_and(|session| session.0.as_raw() == next_session)
                    })
                })
            {
                next_session += 1;
            }

            session::SessionId::from_raw(next_session)
        }
    };

    if mode == ecs::SeatKind::VtBound {
        seat::vt_open(session_id.as_raw())?;
    }

    world.entity_mut(client_entity).insert((
        ecs::SeatName(name.clone()),
        ecs::SeatEntity(seat_entity),
        ecs::SessionId(session_id),
        ecs::SeatState(seat::SeatState::Enabled),
    ));
    if let Some(mut clients) = world.get_mut::<ecs::SeatClients>(seat_entity) {
        clients.0.push(client_entity);
    }
    world
        .entity_mut(seat_entity)
        .insert(ecs::ActiveClient(client_entity));
    seat::notify_client_added(world, seat_entity, client_entity);
    notify_state(world, client_entity);
    notify_active(world, seat_entity, client_entity);

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

    if let Some(active_client) = world.get::<ecs::ActiveClient>(seat_entity) {
        let active_state = world
            .get::<ecs::SeatState>(active_client.0)
            .map(|state| state.0);
        if !seat::is_vt_bound(world, seat_entity)
            || active_state != Some(seat::SeatState::PendingDisabled)
        {
            log::error!("Could not enable client: seat already has an active client");
            return Err(anyhow::anyhow!("seat already has an active client"));
        }
    }

    if seat::is_vt_bound(world, seat_entity) {
        seat::vt_open(session_id.as_raw())?;
    }

    if let Some(devices) = world.get_mut::<ecs::Devices>(client_entity) {
        for device_entity in devices.0.clone() {
            if let Err(err) = device::activate(world, device_entity) {
                let path = device::path_display(world, device_entity);
                log::error!("Could not activate {path}: {err}");
            }
        }
    }

    if let Some(object) = world.get::<ecs::SeatObject>(client_entity) {
        object.0.send_enabled();
    }
    world
        .entity_mut(client_entity)
        .insert(ecs::SeatState(seat::SeatState::Enabled));
    world
        .entity_mut(seat_entity)
        .insert(ecs::ActiveClient(client_entity));
    notify_state(world, client_entity);
    notify_active(world, seat_entity, client_entity);
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

    if !client::is_active(world, seat_entity, client_entity) {
        log::error!("Could not disable client: client is not the active seat client");
        return Err(anyhow::anyhow!("client is not the active seat client"));
    }

    let session_id = session_id(world, client_entity)?;
    let name = seat::name(world, seat_entity)?;

    deactivate_devices(world, client_entity);

    world
        .entity_mut(client_entity)
        .insert(ecs::SeatState(seat::SeatState::PendingDisabled));
    notify_state(world, client_entity);
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
    world
        .entity_mut(client_entity)
        .insert(ecs::SeatState(seat::SeatState::Disabled));
    notify_state(world, client_entity);
    log::info!("Disabled client {} on {}", session_id.as_raw(), name);

    if !client::is_active(world, seat_entity, client_entity) {
        return Ok(());
    }

    world.entity_mut(seat_entity).remove::<ecs::ActiveClient>();
    notify_active(world, seat_entity, client_entity);
    let _ = seat::activate(world, seat_entity);

    Ok(())
}
