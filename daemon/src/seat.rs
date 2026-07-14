use crate::seat_v1::{seat, seat_manager};
use crate::session_core_v1::{
    session_core_seat, session_core_seat_client, session_core_seat_device,
};
use crate::{cgroup, device, ecs, seat_v1, session, session_core_v1, terminal};
use bevy_ecs::{entity, world};
use calloop::signals;
use rustix::io;
use std::fs;

const MAX_SEAT_DEVICES: u8 = 128;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SeatState {
    New,
    Enabled,
    PendingDisabled,
    Disabled,
}

pub fn client_entity_by_seat_object(
    world: &world::World,
    entities: &[entity::Entity],
    object: &seat_v1::seat::Seat,
) -> Option<entity::Entity> {
    entities.iter().copied().find(|entity| {
        world
            .get::<ecs::SeatObject>(*entity)
            .is_some_and(|seat_object| seat_object.0 == *object)
    })
}

pub fn client_entity_by_device_object(
    world: &world::World,
    entities: &[entity::Entity],
    object: &seat_v1::device::Device,
) -> Option<entity::Entity> {
    entities.iter().copied().find(|entity| {
        world.get::<ecs::Devices>(*entity).is_some_and(|devices| {
            devices
                .0
                .iter()
                .any(|device_entity| device::contains_object(world, *device_entity, object))
        })
    })
}

pub fn add_client(
    world: &mut world::World,
    seat_entity: entity::Entity,
    client_entity: entity::Entity,
) -> anyhow::Result<()> {
    if world.get::<ecs::SeatName>(client_entity).is_some() {
        log::error!("Could not add client: client is already a member of a seat");
        return Err(anyhow::anyhow!("client is already a member of a seat"));
    }
    if world.get::<ecs::Session>(client_entity).is_some() {
        log::error!("Could not add client: client cannot be reused");
        return Err(anyhow::anyhow!("client cannot be reused"));
    }

    let seat_name = seat_name(world, seat_entity)?;
    refresh_current_vts(world, &[seat_entity]);

    let mode = seat_mode(world, seat_entity)?;
    let session_id = match mode {
        ecs::SeatKind::VtBound => {
            if let Some(active_client) = world.get::<ecs::ActiveClient>(seat_entity) {
                let active_state = world
                    .get::<ecs::SeatState>(active_client.0)
                    .map(|state| state.0);
                if active_state != Some(SeatState::PendingDisabled) {
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
                                .get::<ecs::Session>(*client)
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
                            .get::<ecs::Session>(*client)
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
        vt_open(session_id.as_raw())?;
    }

    world.entity_mut(client_entity).insert((
        ecs::SeatName(seat_name.clone()),
        ecs::SeatEntity(seat_entity),
        ecs::Session(session_id),
        ecs::SeatState(SeatState::Enabled),
    ));
    if let Some(mut clients) = world.get_mut::<ecs::SeatClients>(seat_entity) {
        clients.0.push(client_entity);
    }
    world
        .entity_mut(seat_entity)
        .insert(ecs::ActiveClient(client_entity));

    if let Some(object) = world.get::<ecs::SeatObject>(client_entity) {
        object.0.send_enabled();
    }
    log::info!(
        session_id = session_id.as_raw(),
        seat = seat_name.as_str(),
        pid = world
            .get::<ecs::ClientPid>(client_entity)
            .map_or(0, |pid| pid.0);
        "added client to seat"
    );

    Ok(())
}

pub fn is_active_client(
    world: &world::World,
    seat_entity: entity::Entity,
    client_entity: entity::Entity,
) -> bool {
    world
        .get::<ecs::ActiveClient>(seat_entity)
        .is_some_and(|active_client| active_client.0 == client_entity)
}

pub fn open_client(
    world: &mut world::World,
    seat_entity: entity::Entity,
    client_entity: entity::Entity,
) -> anyhow::Result<()> {
    let seat_name = seat_name(world, seat_entity)?;
    let session_id = client_session_id(world, client_entity)?;
    let state = seat_state(world, client_entity)?;

    if state != SeatState::New && state != SeatState::Disabled {
        log::error!("Could not enable client: client is not new or disabled");
        return Err(anyhow::anyhow!("client is not new or disabled"));
    }

    if let Some(active_client) = world.get::<ecs::ActiveClient>(seat_entity) {
        let active_state = world
            .get::<ecs::SeatState>(active_client.0)
            .map(|state| state.0);
        if !is_vt_bound(world, seat_entity) || active_state != Some(SeatState::PendingDisabled) {
            log::error!("Could not enable client: seat already has an active client");
            return Err(anyhow::anyhow!("seat already has an active client"));
        }
    }

    if is_vt_bound(world, seat_entity) {
        vt_open(session_id.as_raw())?;
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
        .insert(ecs::SeatState(SeatState::Enabled));
    world
        .entity_mut(seat_entity)
        .insert(ecs::ActiveClient(client_entity));
    log::info!("Opened client {} on {}", session_id.as_raw(), seat_name);

    Ok(())
}

pub fn disable_client(
    world: &mut world::World,
    seat_entity: entity::Entity,
    client_entity: entity::Entity,
) -> anyhow::Result<()> {
    if seat_state(world, client_entity)? != SeatState::Enabled {
        log::error!("Could not disable client: client is not active");
        return Err(anyhow::anyhow!("client is not active"));
    }

    if !is_active_client(world, seat_entity, client_entity) {
        log::error!("Could not disable client: client is not the active seat client");
        return Err(anyhow::anyhow!("client is not the active seat client"));
    }

    let session_id = client_session_id(world, client_entity)?;
    let seat_name = seat_name(world, seat_entity)?;

    deactivate_client_devices(world, client_entity);

    world
        .entity_mut(client_entity)
        .insert(ecs::SeatState(SeatState::PendingDisabled));
    if let Some(object) = world.get::<ecs::SeatObject>(client_entity) {
        object.0.send_disabled();
    }
    log::info!("Disabling client {} on {}", session_id.as_raw(), seat_name);

    Ok(())
}

pub fn ack_disable_client(
    world: &mut world::World,
    seat_entity: entity::Entity,
    client_entity: entity::Entity,
) -> anyhow::Result<()> {
    if seat_state(world, client_entity)? != SeatState::PendingDisabled {
        log::error!("Could not ack disable: client is not pending disable");
        return Err(anyhow::anyhow!("client is not pending disable"));
    }

    let session_id = client_session_id(world, client_entity)?;
    let seat_name = seat_name(world, seat_entity)?;
    world
        .entity_mut(client_entity)
        .insert(ecs::SeatState(SeatState::Disabled));
    log::info!("Disabled client {} on {}", session_id.as_raw(), seat_name);

    if !is_active_client(world, seat_entity, client_entity) {
        return Ok(());
    }

    world.entity_mut(seat_entity).remove::<ecs::ActiveClient>();
    let _ = activate(world, seat_entity);

    Ok(())
}

fn deactivate_client_devices(world: &mut world::World, client_entity: entity::Entity) {
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

        match device::device_kind(world, device_entity) {
            Ok(device::DeviceKind::Drm | device::DeviceKind::Wscons) => {
                retained_devices.push(device_entity);
            }
            Ok(device::DeviceKind::Evdev | device::DeviceKind::Hidraw) => {
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

pub fn set_next_session(
    world: &mut world::World,
    seat_entity: entity::Entity,
    client_entity: entity::Entity,
    session: u32,
) -> anyhow::Result<()> {
    if seat_state(world, client_entity)? != SeatState::Enabled {
        log::error!("Could not set next session: client is not active");
        return Err(anyhow::anyhow!("client is not active"));
    }

    if !is_active_client(world, seat_entity, client_entity) {
        log::error!("Could not set next session: client is not the active seat client");
        return Err(anyhow::anyhow!("client is not the active seat client"));
    }

    if session == 0 {
        log::error!("Could not set next session: invalid session value {session}");
        return Err(anyhow::anyhow!("invalid session value"));
    }
    let session = session::SessionId::from_raw(session);

    if client_session_id(world, client_entity)? == session {
        log::info!("Could not set next session: requested session is already active");
        return Ok(());
    }

    if world.get::<ecs::NextClient>(seat_entity).is_some() {
        log::info!("Could not set next session: switch is already queued");
        return Ok(());
    }

    if is_vt_bound(world, seat_entity) {
        let cur_vt = world
            .get::<ecs::CurrentVt>(seat_entity)
            .map_or(0, |vt| vt.0);
        log::info!("Switching from VT {} to VT {}", cur_vt, session.as_raw());
        vt_switch(cur_vt, session.as_raw())?;
        return Ok(());
    }

    let Some(target) = world
        .get::<ecs::SeatClients>(seat_entity)
        .and_then(|clients| {
            clients.0.iter().copied().find(|candidate| {
                world
                    .get::<ecs::Session>(*candidate)
                    .is_some_and(|candidate_session| candidate_session.0 == session)
            })
        })
    else {
        log::error!("Could not set next session: no such client");
        return Err(anyhow::anyhow!("no such client"));
    };

    let seat_name = seat_name(world, seat_entity)?;
    log::info!(
        "Queuing switch to client {} on {}",
        session.as_raw(),
        seat_name
    );
    world
        .entity_mut(seat_entity)
        .insert(ecs::NextClient(target));
    disable_client(world, seat_entity, client_entity)?;

    Ok(())
}

pub fn remove_client(
    world: &mut world::World,
    seat_entity: entity::Entity,
    client_entity: entity::Entity,
) -> anyhow::Result<()> {
    if world
        .get::<ecs::NextClient>(seat_entity)
        .is_some_and(|next_client| next_client.0 == client_entity)
    {
        world.entity_mut(seat_entity).remove::<ecs::NextClient>();
    }

    let session_id = world
        .get::<ecs::Session>(client_entity)
        .map(|session| session.0);
    let seat_name = seat_name(world, seat_entity)?;

    if let Some(mut clients) = world.get_mut::<ecs::SeatClients>(seat_entity) {
        clients.0.retain(|client| *client != client_entity);
    }

    if let Some(devices) = world.get::<ecs::Devices>(client_entity) {
        for device_entity in devices.0.clone() {
            if let Err(err) = device::deactivate(world, device_entity) {
                let path = device::path_display(world, device_entity);
                log::error!("Could not deactivate {path}: {err}");
            }
            world.despawn(device_entity);
        }
    }
    if let Some(mut devices) = world.get_mut::<ecs::Devices>(client_entity) {
        devices.0.clear();
    }

    let was_current = is_active_client(world, seat_entity, client_entity);
    if was_current {
        world.entity_mut(seat_entity).remove::<ecs::ActiveClient>();
        let _ = activate(world, seat_entity);
    }

    if is_vt_bound(world, seat_entity) {
        let has_active_client = world.get::<ecs::ActiveClient>(seat_entity).is_some();
        if was_current && !has_active_client {
            log::debug!("Closing active VT");
            let vt = session_id.map(session::SessionId::as_raw).or_else(|| {
                world
                    .get::<ecs::CurrentVt>(seat_entity)
                    .map(|current| current.0)
                    .filter(|current| *current != 0)
            });
            if let Some(vt) = vt {
                vt_close(vt)?;
            } else {
                log::warn!("Could not determine VT for removed active client");
            }
        } else if let Some(session_id) = session_id
            && !was_current
        {
            log::debug!("Closing inactive VT");
            vt_close(session_id.as_raw())?;
        }
    }

    world.despawn(client_entity);

    log::info!(
        "Removed client {} from {}",
        session_id.map_or(0, session::SessionId::as_raw),
        seat_name
    );

    Ok(())
}

pub fn open_device(
    world: &mut world::World,
    seat_entity: entity::Entity,
    client_entity: entity::Entity,
    device: seat_v1::device::Device,
    path: &str,
) {
    if !is_active_client(world, seat_entity, client_entity) {
        device.send_failed(
            seat_v1::DeviceError::SeatDisabled,
            io::Errno::PERM.raw_os_error().cast_unsigned(),
            "open_device is only valid for the active seat client",
        );
        return;
    }

    let Ok(sanitized_path) = fs::canonicalize(path) else {
        device.send_failed(
            seat_v1::DeviceError::OpenFailed,
            io::Errno::NOENT.raw_os_error().cast_unsigned(),
            "could not canonicalize device path",
        );
        return;
    };

    if seat_state(world, client_entity).ok() != Some(SeatState::Enabled) {
        device.send_failed(
            seat_v1::DeviceError::SeatDisabled,
            io::Errno::BUSY.raw_os_error().cast_unsigned(),
            "open_device is only valid while the seat is enabled",
        );
        return;
    }

    let Some(kind) = device::DeviceKind::from_path(&sanitized_path) else {
        log::error!(
            "{} is not a supported device type ",
            sanitized_path.display()
        );
        device.send_failed(
            seat_v1::DeviceError::OpenFailed,
            io::Errno::INVAL.raw_os_error().cast_unsigned(),
            "unsupported device type",
        );
        return;
    };

    let Some(device_entities) = world
        .get::<ecs::Devices>(client_entity)
        .map(|devices| devices.0.clone())
    else {
        device.send_failed(
            seat_v1::DeviceError::OpenFailed,
            io::Errno::INVAL.raw_os_error().cast_unsigned(),
            "client has no device storage",
        );
        return;
    };

    let open_handle_count = device_entities
        .iter()
        .map(|device_entity| device::object_count(world, *device_entity))
        .sum::<usize>();
    if open_handle_count >= usize::from(MAX_SEAT_DEVICES) {
        log::error!("Client exceeded max seat devices");
        device.send_failed(
            seat_v1::DeviceError::OpenFailed,
            io::Errno::MFILE.raw_os_error().cast_unsigned(),
            "client exceeded max seat devices",
        );
        return;
    }

    let device_id = device_entities
        .iter()
        .filter_map(|device_entity| device::max_id(world, *device_entity))
        .max()
        .map_or(1, |id| id + 1);

    if let Some(old_device) = device_entities
        .iter()
        .find(|device_entity| device::matches_path(world, **device_entity, &sanitized_path))
        .copied()
    {
        device::add_object(world, old_device, device, device_id);
        if let Err(err) = device::send_opened(world, old_device) {
            log::error!("Could not send opened device event: {err}");
        }
        return;
    }

    let session_id = client_session_id(world, client_entity)
        .map(super::session::SessionId::as_raw)
        .unwrap_or_default();
    let seat_name = seat_name(world, seat_entity).unwrap_or_else(|_| "<unknown>".to_string());

    log::debug!(
        "Opening device {} for client {} on {}",
        sanitized_path.display(),
        session_id,
        seat_name
    );

    let fd = match rustix::fs::open(
        &sanitized_path,
        rustix::fs::OFlags::RDWR
            | rustix::fs::OFlags::NOCTTY
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC
            | rustix::fs::OFlags::NONBLOCK,
        rustix::fs::Mode::empty(),
    ) {
        Ok(fd) => fd,
        Err(err) => {
            log::error!("Could not open file: {err}");
            device.send_failed(
                seat_v1::DeviceError::OpenFailed,
                err.raw_os_error().cast_unsigned(),
                "could not open device",
            );
            return;
        }
    };

    let device_entity = ecs::insert_device(
        world,
        device,
        sanitized_path,
        kind,
        device_id,
        matches!(kind, device::DeviceKind::Evdev | device::DeviceKind::Hidraw),
        fd,
    );

    if let Err(err) = device::activate(world, device_entity) {
        let path = device::path_display(world, device_entity);
        log::error!("Could not activate {path}: {err}");
        let errno = err
            .root_cause()
            .downcast_ref::<io::Errno>()
            .copied()
            .unwrap_or(io::Errno::INVAL);
        if let Some(object) = device::last_object(world, device_entity) {
            object.send_failed(
                seat_v1::DeviceError::OpenFailed,
                errno.raw_os_error().cast_unsigned(),
                "could not activate device",
            );
        }
        world.despawn(device_entity);
        return;
    }

    if let Err(err) = device::send_opened(world, device_entity) {
        log::error!("Could not send opened device event: {err}");
    }
    if let Some(mut devices) = world.get_mut::<ecs::Devices>(client_entity) {
        devices.0.push(device_entity);
    }
}

pub fn refresh_current_vts(world: &mut world::World, seat_entities: &[entity::Entity]) {
    for seat_entity in seat_entities.iter().copied() {
        if !is_vt_bound(world, seat_entity) {
            continue;
        }

        match current_vt() {
            Ok(vt) => {
                world.entity_mut(seat_entity).insert(ecs::CurrentVt(vt));
            }
            Err(err) => log::error!("Could not query active VT: {err}"),
        }
    }
}

pub fn handle_vt_signal(
    world: &mut world::World,
    seat_entities: &[entity::Entity],
    signal: signals::Signal,
) -> Vec<(u32, session_core_v1::ActiveState)> {
    let mut session_updates = Vec::new();

    for seat_entity in seat_entities.iter().copied() {
        if !is_vt_bound(world, seat_entity) {
            continue;
        }

        match signal {
            signals::Signal::SIGUSR1 => match release_vt(world, seat_entity) {
                Ok(Some(vt)) => {
                    session_updates.push((vt, session_core_v1::ActiveState::Online));
                }
                Ok(None) => {}
                Err(err) => {
                    log::error!("Could not release VT: {err}");
                }
            },
            signals::Signal::SIGUSR2 => match activate_vt(world, seat_entity) {
                Ok(Some(vt)) => {
                    session_updates.push((vt, session_core_v1::ActiveState::Active));
                }
                Ok(None) => {}
                Err(err) => {
                    log::error!("Could not activate VT: {err}");
                }
            },
            _ => unreachable!(),
        }
    }

    session_updates
}

pub fn sync_states(
    world: &mut world::World,
    seat_entities: &[entity::Entity],
    session_entities: &[entity::Entity],
) {
    let active_vts = seat_entities
        .iter()
        .copied()
        .filter_map(|seat_entity| {
            let mode = world.get::<ecs::SeatMode>(seat_entity)?;
            if mode.0 != ecs::SeatKind::VtBound {
                return None;
            }

            let vt = world.get::<ecs::CurrentVt>(seat_entity)?.0;
            (vt != 0).then_some(vt)
        })
        .collect::<Vec<_>>();

    if active_vts.is_empty() {
        return;
    }

    let updates = session::entities(world, session_entities)
        .filter_map(|entity| {
            let active_state = world.get::<ecs::ActiveState>(entity)?.0;
            if matches!(
                active_state,
                session_core_v1::ActiveState::Opening | session_core_v1::ActiveState::Closing
            ) {
                return None;
            }

            let vt = world.get::<ecs::Vt>(entity)?.0?;
            let state = if active_vts.contains(&vt) {
                session_core_v1::ActiveState::Active
            } else {
                session_core_v1::ActiveState::Online
            };

            (active_state != state).then_some((entity, state))
        })
        .collect::<Vec<_>>();

    for (entity, state) in updates {
        let Some(session_id) = session::id(world, entity) else {
            continue;
        };
        session::set_state(world, session_id, entity, state);
    }
}

fn send_session_switched(world: &world::World, seat_entity: entity::Entity, session: u32) {
    let Some(clients) = world.get::<ecs::SeatClients>(seat_entity) else {
        return;
    };

    for client in &clients.0 {
        if let Some(object) = world.get::<ecs::SeatObject>(*client) {
            object.0.send_session_switched(session);
        }
    }
}

fn activate(world: &mut world::World, seat_entity: entity::Entity) -> anyhow::Result<()> {
    if world.get::<ecs::ActiveClient>(seat_entity).is_some() {
        return Ok(());
    }

    let seat_name = seat_name(world, seat_entity)?;
    let next_client =
        if let Some(next_client) = world.entity_mut(seat_entity).take::<ecs::NextClient>() {
            log::debug!("Activating next queued client on {seat_name}");
            Some(next_client.0)
        } else {
            let clients = world
                .get::<ecs::SeatClients>(seat_entity)
                .map(|clients| clients.0.clone())
                .unwrap_or_default();
            if clients.is_empty() {
                log::info!("No clients on {seat_name} to activate");
                return Err(anyhow::anyhow!("No clients on {seat_name} to activate"));
            }

            if is_vt_bound(world, seat_entity) {
                let cur_vt = world
                    .get::<ecs::CurrentVt>(seat_entity)
                    .map_or(0, |vt| vt.0);
                let client = clients.iter().copied().find(|client| {
                    world
                        .get::<ecs::Session>(*client)
                        .is_some_and(|session| session.0.as_raw() == cur_vt)
                });
                if client.is_some() {
                    log::debug!("Activating client belonging to VT {cur_vt}");
                } else {
                    log::info!("No clients belonging to VT {cur_vt} to activate");
                    return Err(anyhow::anyhow!(
                        "No clients belonging to VT {cur_vt} to activate"
                    ));
                }
                client
            } else {
                log::debug!("Activating first client on {seat_name}");
                clients.first().copied()
            }
        };

    if let Some(client) = next_client {
        open_client(world, seat_entity, client)?;
    }

    Ok(())
}

fn release_vt(
    world: &mut world::World,
    seat_entity: entity::Entity,
) -> anyhow::Result<Option<u32>> {
    refresh_current_vts(world, &[seat_entity]);
    let cur_vt = world
        .get::<ecs::CurrentVt>(seat_entity)
        .map_or(0, |vt| vt.0);
    let released_vt = world
        .get::<ecs::ActiveClient>(seat_entity)
        .and_then(|client| world.get::<ecs::Session>(client.0))
        .map_or(cur_vt, |session| session.0.as_raw());
    log::debug!("Releasing VT");

    if let Some(active_client) = world
        .get::<ecs::ActiveClient>(seat_entity)
        .map(|client| client.0)
    {
        disable_client(world, seat_entity, active_client)?;
    }

    ack_vt_release(world, seat_entity)?;

    Ok(Some(released_vt))
}

fn ack_vt_release(world: &mut world::World, seat_entity: entity::Entity) -> anyhow::Result<()> {
    let cur_vt = world
        .get::<ecs::CurrentVt>(seat_entity)
        .map_or(0, |vt| vt.0);
    if cur_vt == 0 {
        return Ok(());
    }

    terminal::Terminal::open(cur_vt)?.ack_release()?;
    world.entity_mut(seat_entity).insert(ecs::CurrentVt(0));

    Ok(())
}

fn activate_vt(
    world: &mut world::World,
    seat_entity: entity::Entity,
) -> anyhow::Result<Option<u32>> {
    let acquired_vt = current_vt()?;
    log::debug!("Activating VT");
    terminal::Terminal::open(acquired_vt)?.ack_acquire()?;
    world
        .entity_mut(seat_entity)
        .insert(ecs::CurrentVt(acquired_vt));

    if world.get::<ecs::ActiveClient>(seat_entity).is_none() {
        let has_clients = world
            .get::<ecs::SeatClients>(seat_entity)
            .is_some_and(|clients| !clients.0.is_empty());
        if has_clients {
            activate(world, seat_entity)?;
        } else {
            let seat_name = seat_name(world, seat_entity)?;
            log::debug!("No clients on {seat_name} while acquiring VT {acquired_vt}");
        }
    }

    send_session_switched(world, seat_entity, acquired_vt);

    Ok(Some(acquired_vt))
}

fn vt_open(vt: u32) -> anyhow::Result<()> {
    let terminal = terminal::Terminal::open(vt)?;
    terminal.set_process_switching(true)?;
    terminal.set_keyboard(false)?;
    terminal.set_graphics(true)?;

    Ok(())
}

fn vt_close(vt: u32) -> anyhow::Result<()> {
    let terminal = terminal::Terminal::open(vt)?;
    terminal.set_process_switching(false)?;
    terminal.set_keyboard(true)?;
    terminal.set_graphics(false)?;

    Ok(())
}

fn vt_switch(current_vt: u32, next_vt: u32) -> anyhow::Result<()> {
    let terminal = terminal::Terminal::open(current_vt)?;
    terminal.set_process_switching(true)?;
    terminal.switch_vt(next_vt)?;

    Ok(())
}

fn current_vt() -> anyhow::Result<u32> {
    let tty0fd = terminal::Terminal::open(0)?;
    Ok(u32::from(tty0fd.current_vt()?))
}

fn is_vt_bound(world: &world::World, seat_entity: entity::Entity) -> bool {
    world
        .get::<ecs::SeatMode>(seat_entity)
        .is_some_and(|mode| mode.0 == ecs::SeatKind::VtBound)
}

fn seat_name(world: &world::World, seat_entity: entity::Entity) -> anyhow::Result<String> {
    world
        .get::<ecs::SeatName>(seat_entity)
        .map(|seat| seat.0.clone())
        .ok_or_else(|| anyhow::anyhow!("configured seat has no name"))
}

fn seat_mode(world: &world::World, seat_entity: entity::Entity) -> anyhow::Result<ecs::SeatKind> {
    world
        .get::<ecs::SeatMode>(seat_entity)
        .map(|mode| mode.0)
        .ok_or_else(|| anyhow::anyhow!("configured seat has no mode"))
}

fn seat_state(world: &world::World, client_entity: entity::Entity) -> anyhow::Result<SeatState> {
    world
        .get::<ecs::SeatState>(client_entity)
        .map(|state| state.0)
        .ok_or_else(|| anyhow::anyhow!("client has no seat state"))
}

fn client_session_id(
    world: &world::World,
    client_entity: entity::Entity,
) -> anyhow::Result<session::SessionId> {
    world
        .get::<ecs::Session>(client_entity)
        .map(|session| session.0)
        .ok_or_else(|| anyhow::anyhow!("client has no session id"))
}

impl<C: crate::cgroup::CgroupManager + 'static> seat_v1::SeatV1Handler for crate::Sessiond<C> {
    fn bind(&mut self, object: seat_manager::SeatManager) {
        let Some(client) = object.client() else {
            object.error(
                io::Errno::PERM.raw_os_error().cast_unsigned(),
                "seat access requires peer credentials",
            );
            return;
        };

        let creds = client.creds();
        if self.seat_access.allows(creds) {
            return;
        }

        log::warn!(
            pid = creds.pid.as_raw_pid(),
            uid = creds.uid.as_raw(),
            gid = creds.gid.as_raw();
            "rejected seat_v1 bind from unauthorized client"
        );
        object.error(
            io::Errno::PERM.raw_os_error().cast_unsigned(),
            "seat access denied",
        );
    }
}

impl<C: crate::cgroup::CgroupManager + 'static> hyprwire::Dispatch<seat_manager::SeatManager>
    for crate::Sessiond<C>
{
    fn event(
        &mut self,
        object: &seat_manager::SeatManager,
        event: <seat_manager::SeatManager as hyprwire::Object>::Event<'_>,
    ) {
        match event {
            seat_manager::Event::OpenSeat { seat } => {
                let Some(server_client) = object.client() else {
                    seat.error(
                        io::Errno::PERM.raw_os_error().cast_unsigned(),
                        "seat access requires peer credentials",
                    );
                    return;
                };

                let creds = server_client.creds();
                if !self.seat_access.allows(creds) {
                    log::warn!(
                        pid = creds.pid.as_raw_pid(),
                        uid = creds.uid.as_raw(),
                        gid = creds.gid.as_raw();
                        "rejected seat open from unauthorized client"
                    );
                    seat.error(
                        io::Errno::PERM.raw_os_error().cast_unsigned(),
                        "seat access denied",
                    );
                    return;
                }

                let opener_pid = creds.pid.as_raw_pid().cast_unsigned();
                let opener_session_id = {
                    let pid = opener_pid;
                    match self.cgroup_manager.find_session_by_pid(pid) {
                        Ok(session_id) => session_id,
                        Err(err) => {
                            log::error!("Could not find session for seat opener {pid}: {err}");
                            None
                        }
                    }
                };

                let Some(seat_entity) = self.seat_entities.first().copied() else {
                    log::error!("No seats configured");
                    seat.send_disabled();
                    return;
                };

                let client_entity = ecs::insert_client(&mut self.world, seat.clone(), opener_pid);
                if let Err(err) = add_client(&mut self.world, seat_entity, client_entity) {
                    log::error!("Could not add client to seat: {err}");
                    seat.send_disabled();
                    self.world.despawn(client_entity);
                    return;
                }

                let Some(seat_name) = self
                    .world
                    .get::<ecs::SeatName>(client_entity)
                    .map(|seat| seat.0.clone())
                else {
                    log::error!("Added client has no seat");
                    seat.send_disabled();
                    let _ = remove_client(&mut self.world, seat_entity, client_entity);
                    return;
                };

                self.client_entities.push(client_entity);

                if let Some(session_id) = opener_session_id {
                    if let Some(entity) =
                        session::entity(&self.world, &self.session_entities, session_id)
                    {
                        if self
                            .world
                            .get::<ecs::SeatEntity>(entity)
                            .is_none_or(|seat| seat.0 != seat_entity)
                        {
                            log::info!(session_id = session_id.as_raw(), seat = seat_name.as_str(); "session seat changed");
                            self.world.entity_mut(entity).insert((
                                ecs::SeatName(seat_name.clone()),
                                ecs::SeatEntity(seat_entity),
                            ));
                        }
                        if let Some(objects) = self.world.get::<ecs::SessionObjects>(entity) {
                            for object in &objects.0 {
                                object.send_seat(&seat_name);
                            }
                        }
                    } else {
                        log::warn!(session_id = session_id.as_raw(); "seat opened for unknown session");
                    }
                }
            }
            seat_manager::Event::Destroy => {}
        }
    }
}

impl<C: crate::cgroup::CgroupManager + 'static> hyprwire::Dispatch<seat::Seat>
    for crate::Sessiond<C>
{
    fn event(&mut self, object: &seat::Seat, event: <seat::Seat as hyprwire::Object>::Event<'_>) {
        let Some(client_entity) =
            client_entity_by_seat_object(&self.world, &self.client_entities, object)
        else {
            return;
        };
        let Some(seat_entity) = self
            .world
            .get::<ecs::SeatEntity>(client_entity)
            .map(|seat| seat.0)
        else {
            log::error!("Seat client has no seat entity");
            return;
        };

        match event {
            seat::Event::AckDisabled => {
                let state = self
                    .world
                    .get::<ecs::SeatState>(client_entity)
                    .map(|state| state.0);
                if let Err(err) = ack_disable_client(&mut self.world, seat_entity, client_entity) {
                    log::error!("Could not ack disable: {err}");

                    match state {
                        Some(SeatState::Enabled) => object.send_seat_error(
                            seat_v1::SeatError::InvalidAckDisabled,
                            "ack_disabled was sent while enabled",
                        ),
                        Some(SeatState::Disabled) => object.send_seat_error(
                            seat_v1::SeatError::AlreadyDisabled,
                            "disabled state was already acknowledged",
                        ),
                        _ => {}
                    }
                }
            }
            seat::Event::OpenDevice { device, path } => {
                open_device(&mut self.world, seat_entity, client_entity, device, &path);
            }
            seat::Event::SwitchSession { session } => {
                if !is_vt_bound(&self.world, seat_entity) {
                    object.send_seat_error(
                        seat_v1::SeatError::InvalidSwitchSession,
                        "session switching is not supported",
                    );
                    return;
                }

                if let Err(err) =
                    set_next_session(&mut self.world, seat_entity, client_entity, session)
                {
                    log::error!("Could not switch session: {err}");
                }
            }
            seat::Event::Destroy => {
                if let Err(err) = remove_client(&mut self.world, seat_entity, client_entity) {
                    log::error!("Could not remove client from seat: {err}");
                }
                self.client_entities
                    .retain(|entity| *entity != client_entity);
            }
        }
    }
}

impl<C: cgroup::CgroupManager + 'static> hyprwire::Dispatch<session_core_seat::SessionCoreSeat>
    for crate::Sessiond<C>
{
    fn event(
        &mut self,
        _object: &session_core_seat::SessionCoreSeat,
        event: <session_core_seat::SessionCoreSeat as hyprwire::Object>::Event<'_>,
    ) {
        match event {
            session_core_seat::Event::GetClient {
                session_core_seat_client,
                pid,
            } => {}
            session_core_seat::Event::Destroy => {}
        }
    }
}

impl<C: cgroup::CgroupManager + 'static>
    hyprwire::Dispatch<session_core_seat_client::SessionCoreSeatClient> for crate::Sessiond<C>
{
    fn event(
        &mut self,
        _object: &session_core_seat_client::SessionCoreSeatClient,
        event: <session_core_seat_client::SessionCoreSeatClient as hyprwire::Object>::Event<'_>,
    ) {
        match event {
            session_core_seat_client::Event::GetDevice {
                session_core_seat_device,
                device_id,
            } => {}
            session_core_seat_client::Event::Destroy => {}
        }
    }
}

impl<C: cgroup::CgroupManager + 'static>
    hyprwire::Dispatch<session_core_seat_device::SessionCoreSeatDevice> for crate::Sessiond<C>
{
    fn event(
        &mut self,
        _object: &session_core_seat_device::SessionCoreSeatDevice,
        event: <session_core_seat_device::SessionCoreSeatDevice as hyprwire::Object>::Event<'_>,
    ) {
        let session_core_seat_device::Event::Destroy = event;
    }
}
