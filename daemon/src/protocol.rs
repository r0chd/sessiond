use crate::session_core_v1::{session_core_session, session_core_user};
use bevy_ecs::prelude::*;
use std::collections::HashMap;

#[derive(Resource, Default)]
pub struct ProtocolObjects {
    pub sessions: HashMap<Entity, Vec<session_core_session::SessionCoreSession>>,
    pub users: HashMap<Entity, Vec<session_core_user::SessionCoreUser>>,
}

impl ProtocolObjects {
    pub fn session_objects(
        &self,
        entity: Entity,
    ) -> Option<&[session_core_session::SessionCoreSession]> {
        self.sessions.get(&entity).map(Vec::as_slice)
    }

    pub fn add_session_object(
        &mut self,
        entity: Entity,
        object: session_core_session::SessionCoreSession,
    ) {
        self.sessions.entry(entity).or_default().push(object);
    }

    pub fn remove_session_object(
        &mut self,
        entity: Entity,
        object: &session_core_session::SessionCoreSession,
    ) {
        if let Some(objects) = self.sessions.get_mut(&entity) {
            objects.retain(|candidate| candidate != object);
        }
    }

    pub fn remove_session_objects(&mut self, entity: Entity) {
        self.sessions.remove(&entity);
    }

    pub fn session_entity_by_object(
        &self,
        object: &session_core_session::SessionCoreSession,
    ) -> Option<Entity> {
        self.sessions.iter().find_map(|(entity, objects)| {
            objects
                .iter()
                .any(|candidate| candidate == object)
                .then_some(*entity)
        })
    }

    pub fn user_objects(&self, entity: Entity) -> Option<&[session_core_user::SessionCoreUser]> {
        self.users.get(&entity).map(Vec::as_slice)
    }

    pub fn add_user_object(&mut self, entity: Entity, object: session_core_user::SessionCoreUser) {
        self.users.entry(entity).or_default().push(object);
    }

    pub fn remove_user_object(
        &mut self,
        entity: Entity,
        object: &session_core_user::SessionCoreUser,
    ) {
        if let Some(objects) = self.users.get_mut(&entity) {
            objects.retain(|candidate| candidate != object);
        }
    }

    pub fn user_entity_by_object(
        &self,
        object: &session_core_user::SessionCoreUser,
    ) -> Option<Entity> {
        self.users.iter().find_map(|(entity, objects)| {
            objects
                .iter()
                .any(|candidate| candidate == object)
                .then_some(*entity)
        })
    }
}
