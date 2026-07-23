use crate::seat_v1::{seat, seat_manager};
use crate::session_core_v1::session_core_seat;
use crate::{client, device, ecs, seat_v1, session, session_core_v1, terminal, user};
use bevy_ecs::{entity, world};
use calloop::signals;
use rustix::io;
use std::num;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SeatState {
    New,
    Enabled,
    PendingDisabled,
    Disabled,
}

pub fn spawn(
    world: &mut world::World,
    name: String,
    mode: ecs::SeatKind,
    current_vt: u32,
) -> entity::Entity {
    ecs::ensure_protocol_objects(world);
    let mut cmd = world.spawn(ecs::SeatBundle {
        marker: ecs::Seat,
        name: ecs::SeatName(name),
        mode: ecs::SeatMode(mode),
    });

    if let Some(current_vt) = num::NonZeroU32::new(current_vt) {
        cmd.insert(ecs::CurrentVt(current_vt));
    }

    cmd.id()
}

pub fn entity_by_core_object(
    world: &world::World,
    object: &session_core_seat::SessionCoreSeat,
) -> Option<entity::Entity> {
    world
        .resource::<ecs::ProtocolObjects>()
        .seats
        .iter()
        .find_map(|(entity, objects)| {
            objects
                .iter()
                .any(|candidate| candidate == object)
                .then_some(*entity)
        })
}

pub fn entity_by_name(world: &world::World, name: &str) -> Option<entity::Entity> {
    entities(world).find(|entity| {
        world
            .get::<ecs::SeatName>(*entity)
            .is_some_and(|seat_name| seat_name.0 == name)
    })
}

pub fn entities(world: &world::World) -> impl Iterator<Item = entity::Entity> + '_ {
    world
        .iter_entities()
        .filter(bevy_ecs::world::EntityRef::contains::<ecs::Seat>)
        .map(|entity| entity.id())
}

pub fn clients(
    world: &world::World,
    seat_entity: entity::Entity,
) -> impl Iterator<Item = entity::Entity> + '_ {
    world
        .iter_entities()
        .filter(move |entity| {
            entity.contains::<ecs::Client>()
                && entity
                    .get::<ecs::ClientSeat>()
                    .is_some_and(|seat| seat.0 == seat_entity)
        })
        .map(|entity| entity.id())
}

pub fn first(world: &world::World) -> Option<entity::Entity> {
    entities(world).next()
}

const fn mode_event(mode: ecs::SeatKind) -> session_core_v1::SeatMode {
    match mode {
        ecs::SeatKind::VtBound => session_core_v1::SeatMode::VtBound,
        ecs::SeatKind::NonVtBound => session_core_v1::SeatMode::NonVtBound,
    }
}

pub fn add_object(
    world: &mut world::World,
    seat_entity: entity::Entity,
    object: session_core_seat::SessionCoreSeat,
    management_access: crate::Access,
) {
    if let Some(name) = world.get::<ecs::SeatName>(seat_entity) {
        object.send_seat(&name.0);
    }
    if let Some(mode) = world.get::<ecs::SeatMode>(seat_entity) {
        object.send_mode(mode_event(mode.0));
    }
    if let Some(vt) = world.get::<ecs::CurrentVt>(seat_entity) {
        object.send_current_vt(vt.0.get());
    }
    for client_entity in clients(world, seat_entity) {
        if let Some(pid) = world.get::<ecs::ClientPid>(client_entity) {
            object.send_client_added(pid.0);
        }
    }
    if let Some(client) = object.client() {
        object.send_can_modify(u32::from(can_modify(
            world,
            seat_entity,
            client.creds(),
            management_access,
        )));
    }
    object.send_done();

    ecs::ensure_protocol_objects(world);
    world
        .resource_mut::<ecs::ProtocolObjects>()
        .seats
        .entry(seat_entity)
        .or_default()
        .push(object);
}

fn can_modify(
    world: &world::World,
    seat_entity: entity::Entity,
    creds: &rustix::net::UCred,
    management_access: crate::Access,
) -> bool {
    if management_access.allows(creds) {
        return true;
    }

    let Some(name) = world.get::<ecs::SeatName>(seat_entity) else {
        return false;
    };

    session::entities_by_seat_name(world, &name.0).all(|entity| {
        world
            .get::<ecs::Uid>(entity)
            .is_some_and(|uid| uid.0 == creds.uid.as_raw())
    })
}

pub fn notify_client_added(
    world: &world::World,
    seat_entity: entity::Entity,
    client_entity: entity::Entity,
) {
    let Some(pid) = world.get::<ecs::ClientPid>(client_entity).map(|pid| pid.0) else {
        return;
    };
    if let Some(objects) = world
        .resource::<ecs::ProtocolObjects>()
        .seats
        .get(&seat_entity)
    {
        for object in objects {
            object.send_client_added(pid);
        }
    }
}

fn notify_current_vt(world: &world::World, seat_entity: entity::Entity) {
    let vt = world
        .get::<ecs::CurrentVt>(seat_entity)
        .map(|vt| vt.0)
        .map_or(0, num::NonZeroU32::get);
    if let Some(objects) = world
        .resource::<ecs::ProtocolObjects>()
        .seats
        .get(&seat_entity)
    {
        for object in objects {
            object.send_current_vt(vt);
        }
    }
}

pub fn set_next_session(
    world: &mut world::World,
    seat_entity: entity::Entity,
    client_entity: entity::Entity,
    session: u32,
) -> anyhow::Result<()> {
    if state(world, client_entity)? != SeatState::Enabled {
        log::error!("Could not set next session: client is not active");
        return Err(anyhow::anyhow!("client is not active"));
    }

    if !client::is_active(world, seat_entity, client_entity) {
        log::error!("Could not set next session: client is not the active seat client");
        return Err(anyhow::anyhow!("client is not the active seat client"));
    }

    if session == 0 {
        log::error!("Could not set next session: invalid session value {session}");
        return Err(anyhow::anyhow!("invalid session value"));
    }
    let session = session::SessionId::from_raw(session);

    if client::session_id(world, client_entity)? == session {
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
            .map(|vt| vt.0)
            .ok_or_else(|| anyhow::anyhow!("could not determine current VT"))?;
        log::info!("Switching from VT {} to VT {}", cur_vt, session.as_raw());
        vt_switch(cur_vt.get(), session.as_raw())?;
        return Ok(());
    }

    let Some(target) = clients(world, seat_entity).find(|candidate| {
        world
            .get::<ecs::SessionId>(*candidate)
            .is_some_and(|candidate_session| candidate_session.0 == session)
    }) else {
        log::error!("Could not set next session: no such client");
        return Err(anyhow::anyhow!("no such client"));
    };

    let name = name(world, seat_entity)?;
    log::info!("Queuing switch to client {} on {}", session.as_raw(), name);
    world
        .entity_mut(seat_entity)
        .insert(ecs::NextClient(target));
    client::notify_next(world, target, true);
    client::disable(world, seat_entity, client_entity)?;

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
        client::notify_next(world, client_entity, false);
    }

    let session_id = world
        .get::<ecs::SessionId>(client_entity)
        .map(|session| session.0);
    let name = name(world, seat_entity)?;

    let devices = device::devices(world, client_entity).collect::<Vec<_>>();
    for device_entity in devices {
        if let Err(err) = device::deactivate(world, device_entity) {
            let path = device::path_display(world, device_entity);
            log::error!("Could not deactivate {path}: {err}");
        }
        device::notify_removed(world, client_entity, device_entity);
        device::remove_protocol_objects(world, device_entity);
        world.despawn(device_entity);
    }

    let was_current = client::is_active(world, seat_entity, client_entity);
    if was_current {
        world.entity_mut(seat_entity).remove::<ecs::ActiveClient>();
        client::notify_active(world, seat_entity, client_entity);
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
                    .map(num::NonZeroU32::get)
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

    client::notify_destroyed(world, client_entity);
    world
        .resource_mut::<ecs::ProtocolObjects>()
        .clients
        .remove(&client_entity);
    world.despawn(client_entity);

    log::info!(
        "Removed client {} from {}",
        session_id.map_or(0, session::SessionId::as_raw),
        name
    );

    Ok(())
}

pub fn refresh_current_vt(world: &mut world::World, seat_entity: entity::Entity) {
    if !is_vt_bound(world, seat_entity) {
        return;
    }

    match current_vt() {
        Ok(vt) => {
            let changed = world
                .get::<ecs::CurrentVt>(seat_entity)
                .is_none_or(|current| current.0 != vt);
            world.entity_mut(seat_entity).insert(ecs::CurrentVt(vt));
            if changed {
                notify_current_vt(world, seat_entity);
            }
        }
        Err(err) => log::error!("Could not query active VT: {err}"),
    }
}

pub fn refresh_current_vts(world: &mut world::World) {
    let seats = entities(world).collect::<Vec<_>>();
    for seat_entity in seats {
        refresh_current_vt(world, seat_entity);
    }
}

pub fn handle_vt_signal(
    world: &mut world::World,
    signal: signals::Signal,
) -> Vec<(num::NonZeroU32, session_core_v1::ActiveState)> {
    let mut session_updates = Vec::new();

    let seats = entities(world).collect::<Vec<_>>();
    for seat_entity in seats {
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

pub fn sync_states(world: &mut world::World) {
    let active_vts = entities(world)
        .filter_map(|seat_entity| {
            let mode = world.get::<ecs::SeatMode>(seat_entity)?;
            if mode.0 != ecs::SeatKind::VtBound {
                return None;
            }

            Some(world.get::<ecs::CurrentVt>(seat_entity)?.0)
        })
        .collect::<Vec<_>>();

    if active_vts.is_empty() {
        return;
    }

    let updates = session::entities(world)
        .filter_map(|entity| {
            let active_state = world.get::<ecs::ActiveState>(entity)?.0;
            if matches!(
                active_state,
                session_core_v1::ActiveState::Opening | session_core_v1::ActiveState::Closing
            ) {
                return None;
            }

            let vt = world.get::<ecs::Vt>(entity)?.0;
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
    for client in clients(world, seat_entity) {
        if let Some(object) = world.get::<ecs::SeatObject>(client) {
            object.0.send_session_switched(session);
        }
    }
}

pub fn activate(world: &mut world::World, seat_entity: entity::Entity) -> anyhow::Result<()> {
    if world.get::<ecs::ActiveClient>(seat_entity).is_some() {
        return Ok(());
    }

    let name = name(world, seat_entity)?;
    let next_client =
        if let Some(next_client) = world.entity_mut(seat_entity).take::<ecs::NextClient>() {
            log::debug!("Activating next queued client on {name}");
            client::notify_next(world, next_client.0, false);
            Some(next_client.0)
        } else {
            let clients = clients(world, seat_entity).collect::<Vec<_>>();
            if clients.is_empty() {
                log::info!("No clients on {name} to activate");
                return Err(anyhow::anyhow!("No clients on {name} to activate"));
            }

            if is_vt_bound(world, seat_entity) {
                let cur_vt = world.get::<ecs::CurrentVt>(seat_entity).map(|vt| vt.0);
                let client = clients.iter().copied().find(|client| {
                    world.get::<ecs::SessionId>(*client).is_some_and(|session| {
                        cur_vt.is_some_and(|vt| session.0.as_raw() == vt.get())
                    })
                });
                if client.is_some() {
                    log::debug!(
                        "Activating client belonging to VT {}",
                        cur_vt.map_or(0, num::NonZeroU32::get)
                    );
                } else {
                    let cur_vt = cur_vt.map_or(0, num::NonZeroU32::get);
                    log::info!("No clients belonging to VT {cur_vt} to activate");
                    return Err(anyhow::anyhow!(
                        "No clients belonging to VT {cur_vt} to activate"
                    ));
                }
                client
            } else {
                log::debug!("Activating first client on {name}");
                clients.first().copied()
            }
        };

    if let Some(client) = next_client {
        client::open(world, seat_entity, client)?;
    }

    Ok(())
}

fn release_vt(
    world: &mut world::World,
    seat_entity: entity::Entity,
) -> anyhow::Result<Option<num::NonZeroU32>> {
    refresh_current_vt(world, seat_entity);
    let cur_vt = world.get::<ecs::CurrentVt>(seat_entity).map(|vt| vt.0);
    let released_vt = world
        .get::<ecs::ActiveClient>(seat_entity)
        .and_then(|client| world.get::<ecs::SessionId>(client.0))
        .and_then(|session| num::NonZeroU32::new(session.0.as_raw()))
        .or(cur_vt);
    log::debug!("Releasing VT");

    if let Some(active_client) = world
        .get::<ecs::ActiveClient>(seat_entity)
        .map(|client| client.0)
    {
        client::disable(world, seat_entity, active_client)?;
    }

    ack_vt_release(world, seat_entity)?;

    Ok(released_vt)
}

fn ack_vt_release(world: &mut world::World, seat_entity: entity::Entity) -> anyhow::Result<()> {
    let Some(cur_vt) = world.get::<ecs::CurrentVt>(seat_entity).map(|vt| vt.0) else {
        return Ok(());
    };

    terminal::Terminal::open(cur_vt.get())?.ack_release()?;
    world.entity_mut(seat_entity).remove::<ecs::CurrentVt>();
    notify_current_vt(world, seat_entity);

    Ok(())
}

fn activate_vt(
    world: &mut world::World,
    seat_entity: entity::Entity,
) -> anyhow::Result<Option<num::NonZeroU32>> {
    let acquired_vt = current_vt()?;
    log::debug!("Activating VT");
    terminal::Terminal::open(acquired_vt.get())?.ack_acquire()?;
    world
        .entity_mut(seat_entity)
        .insert(ecs::CurrentVt(acquired_vt));
    notify_current_vt(world, seat_entity);

    if world.get::<ecs::ActiveClient>(seat_entity).is_none() {
        let has_clients = clients(world, seat_entity).next().is_some();
        if has_clients {
            activate(world, seat_entity)?;
        } else {
            let name = name(world, seat_entity)?;
            log::debug!("No clients on {name} while acquiring VT {acquired_vt}");
        }
    }

    send_session_switched(world, seat_entity, acquired_vt.get());

    Ok(Some(acquired_vt))
}

pub fn vt_open(vt: u32) -> anyhow::Result<()> {
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

fn current_vt() -> anyhow::Result<num::NonZeroU32> {
    let tty0fd = terminal::Terminal::open(0)?;
    num::NonZeroU32::new(u32::from(tty0fd.current_vt()?))
        .ok_or_else(|| anyhow::anyhow!("active VT is zero"))
}

pub fn is_vt_bound(world: &world::World, seat_entity: entity::Entity) -> bool {
    world
        .get::<ecs::SeatMode>(seat_entity)
        .is_some_and(|mode| mode.0 == ecs::SeatKind::VtBound)
}

pub fn name(world: &world::World, seat_entity: entity::Entity) -> anyhow::Result<String> {
    world
        .get::<ecs::SeatName>(seat_entity)
        .map(|seat| seat.0.clone())
        .ok_or_else(|| anyhow::anyhow!("configured seat has no name"))
}

pub fn mode(world: &world::World, seat_entity: entity::Entity) -> anyhow::Result<ecs::SeatKind> {
    world
        .get::<ecs::SeatMode>(seat_entity)
        .map(|mode| mode.0)
        .ok_or_else(|| anyhow::anyhow!("configured seat has no mode"))
}

pub fn state(world: &world::World, client_entity: entity::Entity) -> anyhow::Result<SeatState> {
    world
        .get::<ecs::SeatState>(client_entity)
        .map(|state| state.0)
        .ok_or_else(|| anyhow::anyhow!("client has no seat state"))
}

impl seat_v1::SeatV1Handler for crate::Sessiond {
    fn bind(&mut self, object: seat_manager::SeatManager) {
        let Some(client) = object.client() else {
            object.error(
                io::Errno::PERM.raw_os_error().cast_unsigned(),
                "seat access requires peer credentials",
            );
            return;
        };

        let creds = client.creds();
        if self.world.resource::<ecs::AccessRules>().seat.allows(creds) {
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

impl hyprwire::Dispatch<seat_manager::SeatManager> for crate::Sessiond {
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
                if !self.world.resource::<ecs::AccessRules>().seat.allows(creds) {
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

                let Some(seat_entity) = first(&self.world) else {
                    log::error!("No seats configured");
                    seat.send_disabled();
                    return;
                };

                let client_entity =
                    client::spawn(&mut self.world, seat.clone(), opener_pid, opener_session_id);
                if let Err(err) = client::add(&mut self.world, seat_entity, client_entity) {
                    log::error!("Could not add client to seat: {err}");
                    seat.send_disabled();
                    self.world
                        .resource_mut::<ecs::ProtocolObjects>()
                        .clients
                        .remove(&client_entity);
                    self.world.despawn(client_entity);
                    return;
                }

                let Some(name) = self
                    .world
                    .get::<ecs::SeatName>(client_entity)
                    .map(|seat| seat.0.clone())
                else {
                    log::error!("Added client has no seat");
                    seat.send_disabled();
                    let _ = remove_client(&mut self.world, seat_entity, client_entity);
                    return;
                };

                if let Some(session_id) = opener_session_id {
                    if let Some(entity) = session::entity(&self.world, session_id) {
                        if self
                            .world
                            .get::<ecs::SessionSeat>(entity)
                            .is_none_or(|seat| seat.0 != seat_entity)
                        {
                            log::info!(session_id = session_id.as_raw(), seat = name.as_str(); "session seat changed");
                            self.world.entity_mut(entity).insert((
                                ecs::SeatName(name.clone()),
                                ecs::SessionSeat(seat_entity),
                            ));
                        }
                        if let Some(objects) = self
                            .world
                            .resource::<ecs::ProtocolObjects>()
                            .sessions
                            .get(&entity)
                        {
                            for object in objects {
                                object.send_seat(&name);
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

impl hyprwire::Dispatch<seat::Seat> for crate::Sessiond {
    fn event(&mut self, object: &seat::Seat, event: <seat::Seat as hyprwire::Object>::Event<'_>) {
        let Some(client_entity) = client::entity_by_seat_object(&self.world, object) else {
            return;
        };
        let Some(seat_entity) = self
            .world
            .get::<ecs::ClientSeat>(client_entity)
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
                if let Err(err) = client::ack_disable(&mut self.world, seat_entity, client_entity) {
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
                device::open(&mut self.world, seat_entity, client_entity, device, &path);
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
            }
        }
    }
}

impl hyprwire::Dispatch<session_core_seat::SessionCoreSeat> for crate::Sessiond {
    fn event(
        &mut self,
        object: &session_core_seat::SessionCoreSeat,
        event: <session_core_seat::SessionCoreSeat as hyprwire::Object>::Event<'_>,
    ) {
        match event {
            session_core_seat::Event::GetClient {
                session_core_seat_client,
                pid,
            } => {
                let Some(seat_entity) = entity_by_core_object(&self.world, object) else {
                    session_core_seat_client.send_destroyed();
                    return;
                };
                let Some(client_entity) = client::entity_by_pid(&self.world, pid) else {
                    session_core_seat_client.send_destroyed();
                    return;
                };
                if self
                    .world
                    .get::<ecs::ClientSeat>(client_entity)
                    .is_none_or(|seat| seat.0 != seat_entity)
                {
                    session_core_seat_client.send_destroyed();
                    return;
                }

                client::add_object(
                    &mut self.world,
                    seat_entity,
                    client_entity,
                    session_core_seat_client,
                );
            }
            session_core_seat::Event::Terminate => {
                let Some(seat_entity) = entity_by_core_object(&self.world, object) else {
                    object.send_destroyed();
                    return;
                };
                let Some(name) = self
                    .world
                    .get::<ecs::SeatName>(seat_entity)
                    .map(|seat| seat.0.clone())
                else {
                    object.send_destroyed();
                    return;
                };

                let session_entities =
                    session::entities_by_seat_name(&self.world, &name).collect::<Vec<_>>();

                log::info!(seat = name.as_str(), sessions = session_entities.len(); "seat terminate requested");

                let Some(client) = object.client() else {
                    return;
                };
                let management_access = self.world.resource::<ecs::AccessRules>().management;
                if !can_modify(&self.world, seat_entity, client.creds(), management_access) {
                    object.send_error(
                        session_core_v1::ErrorCode::PermissionDenied,
                        "permission denied",
                    );
                    return;
                }

                for entity in session_entities {
                    self.terminate_session(entity);
                }

                user::sync_all(&mut self.world);
                object.send_ok();
            }
            session_core_seat::Event::Destroy => {
                if let Some(seat_entity) = entity_by_core_object(&self.world, object)
                    && let Some(objects) = self
                        .world
                        .resource_mut::<ecs::ProtocolObjects>()
                        .seats
                        .get_mut(&seat_entity)
                {
                    objects.retain(|candidate| candidate != object);
                }
            }
        }
    }
}
