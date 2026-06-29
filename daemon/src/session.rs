use crate::{ecs, session_core_session, session_core_v1};

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

impl hyprwire::Dispatch<session_core_session::SessionCoreSession> for crate::Sessiond {
    fn event(
        &mut self,
        object: &session_core_session::SessionCoreSession,
        event: <session_core_session::SessionCoreSession as hyprwire::Object>::Event<'_>,
    ) {
        let Some((session_id, entity)) = self.world.session_entity_by_object(object) else {
            return;
        };

        if matches!(event, session_core_session::Event::Destroy) {
            if let Some(objects) = self.world.get_mut::<crate::ecs::SessionObjects>(entity) {
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
                session_core_v1::ErrorCode::PermissionDenied as u32,
                "permission denied",
            );
            return;
        }

        match event {
            session_core_session::Event::Terminate => {
                log::info!(session_id = session_id.as_raw(); "terminate requested");
                self.set_session_state(session_id, entity, session_core_v1::ActiveState::Closing);
                if let Err(err) = self.cgroup_manager.terminate_session(session_id) {
                    log::error!(session_id = session_id.as_raw(), err = err.to_string().as_str(); "terminate failed");
                }
                let _ = self.world.remove_session(session_id);
            }
            session_core_session::Event::Kill => {
                log::info!(session_id = session_id.as_raw(); "kill requested");
                self.set_session_state(session_id, entity, session_core_v1::ActiveState::Closing);
                if let Err(err) = self.cgroup_manager.kill_session(session_id) {
                    log::error!(session_id = session_id.as_raw(), err = err.to_string().as_str(); "kill failed");
                }
                let _ = self.world.remove_session(session_id);
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

pub fn set_state(
    world: &mut ecs::World,
    session_id: SessionId,
    entity: ecs::Entity,
    state: session_core_v1::ActiveState,
) {
    let vt = world.get::<ecs::Vt>(entity).map_or(0, |vt| vt.0);
    log::info!(session_id = session_id.as_raw(), vt; "session state changed");
    if let Some(active_state) = world.get_mut::<ecs::ActiveState>(entity) {
        active_state.0 = state;
    }
    if let Some(objects) = world.get::<ecs::SessionObjects>(entity) {
        for object in &objects.0 {
            object.send_active(state);
        }
    }
}
