use crate::ecs;
use crate::session_core_v1::{session_core_manager, session_core_session};
use bevy_ecs::prelude::*;

impl hyprwire::Dispatch<session_core_manager::SessionCoreManager> for crate::SessiondUaccess {
    fn event(
        &mut self,
        object: &session_core_manager::SessionCoreManager,
        event: <session_core_manager::SessionCoreManager as hyprwire::Object>::Event<'_>,
    ) {
        if let session_core_manager::Event::SessionAdded { session_id } = event
            && let Some(session) = object.send_get_session_by_id::<Self>(session_id)
        {
            ecs::insert_session(&mut self.world, session);
            log::debug!(session_id; "session added");
        }
    }
}

impl hyprwire::Dispatch<session_core_session::SessionCoreSession> for crate::SessiondUaccess {
    fn event(
        &mut self,
        object: &session_core_session::SessionCoreSession,
        event: <session_core_session::SessionCoreSession as hyprwire::Object>::Event<'_>,
    ) {
        let Some(entity) = entity_by_object(&mut self.world, object) else {
            return;
        };

        match event {
            session_core_session::Event::Uid { uid } => {
                self.world.entity_mut(entity).insert(ecs::Uid(uid));
                log::debug!(uid; "session UID changed");
            }
            session_core_session::Event::Active { state } => {
                self.world
                    .entity_mut(entity)
                    .insert(ecs::ActiveState(state));
                log::debug!(state:? = state; "session active state changed");
            }
            session_core_session::Event::IsLocal { is_local } => {
                self.world
                    .entity_mut(entity)
                    .insert(ecs::IsLocal(is_local != 0));
                log::debug!(is_local = is_local != 0; "session locality changed");
            }
            session_core_session::Event::Done => {
                self.world.entity_mut(entity).insert(ecs::Done(true));
                log::debug!("session completed");
            }
            session_core_session::Event::Destroyed => {
                let uid = self.world.get::<ecs::Uid>(entity).map(|uid| uid.0);

                self.world.entity_mut(entity).despawn();

                log::info!(uid = uid; "session removed");
            }
            _ => {}
        }
    }
}

pub fn entity_by_object(
    world: &mut World,
    object: &session_core_session::SessionCoreSession,
) -> Option<Entity> {
    let mut sessions = world.query_filtered::<(Entity, &ecs::SessionObject), With<ecs::Session>>();
    sessions
        .iter(world)
        .find_map(|(entity, session_object)| (&session_object.0 == object).then_some(entity))
}
