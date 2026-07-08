use crate::{ecs, seat, session_core_session, session_core_v1};
use std::time;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct SessionId(u32);

impl SessionId {
    pub const fn as_raw(self) -> u32 {
        self.0
    }

    pub const fn from_raw(id: u32) -> Self {
        Self(id)
    }

    pub const fn next(self) -> Self {
        Self(self.0.wrapping_add(1))
    }
}

pub fn entities<'a>(
    world: &'a ecs::World,
    entities: &'a [ecs::Entity],
) -> impl Iterator<Item = ecs::Entity> + 'a {
    entities
        .iter()
        .copied()
        .filter(|entity| world.get::<ecs::Session>(*entity).is_some())
}

pub fn id(world: &ecs::World, entity: ecs::Entity) -> Option<SessionId> {
    world.get::<ecs::Session>(entity).map(|session| session.0)
}

pub fn entity(
    world: &ecs::World,
    entities: &[ecs::Entity],
    session_id: SessionId,
) -> Option<ecs::Entity> {
    entities.iter().copied().find(|entity| {
        world
            .get::<ecs::Session>(*entity)
            .is_some_and(|session| session.0 == session_id)
    })
}

pub fn entity_by_uid(
    world: &ecs::World,
    entities: &[ecs::Entity],
    uid: u32,
) -> Option<ecs::Entity> {
    self::entities(world, entities).find(|entity| {
        world
            .get::<ecs::Uid>(*entity)
            .is_some_and(|session_uid| session_uid.0 == uid)
    })
}

pub fn entity_by_object(
    world: &ecs::World,
    entities: &[ecs::Entity],
    object: &session_core_session::SessionCoreSession,
) -> Option<ecs::Entity> {
    self::entities(world, entities).find(|entity| {
        world
            .get::<ecs::SessionObjects>(*entity)
            .is_some_and(|objects| {
                objects
                    .0
                    .iter()
                    .any(|session_object| session_object == object)
            })
    })
}

impl hyprwire::Dispatch<session_core_session::SessionCoreSession> for crate::Sessiond {
    fn event(
        &mut self,
        object: &session_core_session::SessionCoreSession,
        event: <session_core_session::SessionCoreSession as hyprwire::Object>::Event<'_>,
    ) {
        let Some(entity) = entity_by_object(&self.world, &self.session_entities, object) else {
            return;
        };
        let Some(session_id) = id(&self.world, entity) else {
            return;
        };

        if matches!(event, session_core_session::Event::Destroy) {
            if let Some(mut objects) = self.world.get_mut::<crate::ecs::SessionObjects>(entity) {
                objects.0.retain(|session_object| session_object != object);
            }
            return;
        }

        let Some(client) = object.client() else {
            return;
        };
        let session_uid = self
            .world
            .get::<crate::ecs::Uid>(entity)
            .map_or(0, |uid| uid.0);
        if !client.creds().uid.is_root() && client.creds().uid.as_raw() != session_uid {
            object.send_error(
                session_core_v1::ErrorCode::PermissionDenied,
                "permission denied",
            );
            return;
        }

        match event {
            session_core_session::Event::Terminate => {
                log::info!(session_id = session_id.as_raw(); "terminate requested");
                set_session_state(
                    &mut self.world,
                    session_id,
                    entity,
                    session_core_v1::ActiveState::Closing,
                );
                if let Err(err) = self.cgroup_manager.terminate_session(session_id) {
                    log::error!(session_id = session_id.as_raw(), err = err.to_string().as_str(); "terminate failed");
                }
                #[cfg(feature = "consolekit")]
                self.sync_consolekit_session(session_id, entity);
                let _ = self.remove_session(session_id);
            }
            session_core_session::Event::Kill => {
                log::info!(session_id = session_id.as_raw(); "kill requested");
                set_session_state(
                    &mut self.world,
                    session_id,
                    entity,
                    session_core_v1::ActiveState::Closing,
                );
                if let Err(err) = self.cgroup_manager.kill_session(session_id) {
                    log::error!(session_id = session_id.as_raw(), err = err.to_string().as_str(); "kill failed");
                }
                #[cfg(feature = "consolekit")]
                self.sync_consolekit_session(session_id, entity);
                let _ = self.remove_session(session_id);
            }
            session_core_session::Event::Lock => {
                log::info!(session_id = session_id.as_raw(); "lock requested");
                if let Some(objects) = self.world.get::<crate::ecs::SessionObjects>(entity) {
                    for object in &objects.0 {
                        object.send_locked();
                        object.send_ok();
                    }
                }
            }
            session_core_session::Event::Unlock => {
                log::info!(session_id = session_id.as_raw(); "unlock requested");
                if let Some(objects) = self.world.get::<crate::ecs::SessionObjects>(entity) {
                    for object in &objects.0 {
                        object.send_unlocked();
                        object.send_ok();
                    }
                }
            }
            _ => unreachable!(),
        }
    }
}

pub fn add_session_object(
    world: &mut ecs::World,
    session_id: SessionId,
    entity: ecs::Entity,
    object: session_core_session::SessionCoreSession,
) {
    log::debug!(session_id = session_id.as_raw(), uid = object.client().map_or(0, |c| c.creds().uid.as_raw()); "session info requested");

    object.send_session_id(session_id.as_raw());
    if let Some(uid) = world.get::<ecs::Uid>(entity) {
        object.send_uid(uid.0);
    }
    if let Some(username) = world.get::<ecs::Username>(entity) {
        object.send_username(&username.0);
    }
    if let Some(vt) = world.get::<ecs::Vt>(entity) {
        object.send_vt(vt.0);
    }
    if let Some(active_state) = world.get::<ecs::ActiveState>(entity) {
        object.send_active(active_state.0);
    }
    if let Some(seat) = world.get::<ecs::SeatName>(entity) {
        object.send_seat(&seat.0);
    }
    if let Some(leader_id) = world.get::<ecs::LeaderId>(entity) {
        object.send_leader(leader_id.0.cast_unsigned());
    }
    if let Some(timestamp) = world.get::<ecs::Timestamp>(entity) {
        let timestamp_secs = timestamp
            .0
            .duration_since(time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let timestamp = u32::try_from(timestamp_secs).unwrap_or(u32::MAX);
        object.send_timestamp(timestamp);
    }
    if let Some(is_local) = world.get::<ecs::IsLocal>(entity) {
        object.send_is_local(u32::from(is_local.0));
    }
    if let Some(remote_host) = world.get::<ecs::RemoteHost>(entity) {
        object.send_remote_host(&remote_host.0);
    }
    object.send_done();

    if let Some(mut objects) = world.get_mut::<ecs::SessionObjects>(entity) {
        objects.0.push(object);
    }
}

pub fn set_session_state(
    world: &mut ecs::World,
    session_id: SessionId,
    entity: ecs::Entity,
    state: session_core_v1::ActiveState,
) {
    let vt = world.get::<ecs::Vt>(entity).map_or(0, |vt| vt.0);
    log::info!(session_id = session_id.as_raw(), vt; "session state changed");
    if let Some(mut active_state) = world.get_mut::<ecs::ActiveState>(entity) {
        active_state.0 = state;
    }
    if let Some(objects) = world.get::<ecs::SessionObjects>(entity) {
        for object in &objects.0 {
            object.send_active(state);
        }
    }
}

pub fn sync_vt_session_states(
    world: &mut ecs::World,
    seat_entities: &[ecs::Entity],
    session_entities: &[ecs::Entity],
) {
    seat::refresh_current_vts(world, seat_entities);
    seat::sync_states(world, seat_entities, session_entities);
}

pub fn set_state(
    world: &mut ecs::World,
    session_id: SessionId,
    entity: ecs::Entity,
    state: session_core_v1::ActiveState,
) {
    let vt = world.get::<ecs::Vt>(entity).map_or(0, |vt| vt.0);
    log::info!(session_id = session_id.as_raw(), vt; "session state changed");
    if let Some(mut active_state) = world.get_mut::<ecs::ActiveState>(entity) {
        active_state.0 = state;
    }
    if let Some(objects) = world.get::<ecs::SessionObjects>(entity) {
        for object in &objects.0 {
            object.send_active(state);
        }
    }
}
