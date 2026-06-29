mod components;

use crate::session;
use crate::{session_core_session, session_core_v1};
pub use components::*;
use std::collections::HashMap;
use std::time;

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Entity(usize);

#[derive(Default)]
pub struct World {
    next_entity: usize,
    free_entities: Vec<Entity>,
    session_entities: HashMap<session::SessionId, Entity>,
    session_order: Vec<session::SessionId>,
    uid_components: Vec<Option<Uid>>,
    username_components: Vec<Option<Username>>,
    vt_components: Vec<Option<Vt>>,
    seat_components: Vec<Option<Seat>>,
    timestamp_components: Vec<Option<Timestamp>>,
    leader_id_components: Vec<Option<LeaderId>>,
    session_objects_components: Vec<Option<SessionObjects>>,
    active_state_components: Vec<Option<ActiveState>>,
}

impl World {
    pub fn new() -> Self {
        Self::default()
    }

    fn entity(&mut self) -> Entity {
        if let Some(entity) = self.free_entities.pop() {
            return entity;
        }

        let entity = Entity(self.next_entity);
        self.next_entity += 1;
        entity
    }

    #[allow(clippy::too_many_arguments)]
    pub fn insert_session(
        &mut self,
        session_id: session::SessionId,
        uid: u32,
        username: String,
        vt: u32,
        seat: Option<String>,
        timestamp: time::SystemTime,
        leader_id: i32,
        objects: Vec<session_core_session::SessionCoreSession>,
        active_state: session_core_v1::ActiveState,
    ) -> Entity {
        let entity = self.entity();
        self.session_entities.insert(session_id, entity);
        self.session_order.push(session_id);
        self.insert(entity, Uid(uid));
        self.insert(entity, Username(username));
        self.insert(entity, Vt(vt));
        self.insert(entity, Seat(seat));
        self.insert(entity, Timestamp(timestamp));
        self.insert(entity, LeaderId(leader_id));
        self.insert(entity, SessionObjects(objects));
        self.insert(entity, ActiveState(active_state));
        entity
    }

    pub fn session_entity(&self, session_id: session::SessionId) -> Option<Entity> {
        self.session_entities.get(&session_id).copied()
    }

    pub fn session_ids(&self) -> impl Iterator<Item = session::SessionId> + '_ {
        self.session_order.iter().copied()
    }

    pub fn session_entity_by_uid(&self, uid: u32) -> Option<(session::SessionId, Entity)> {
        self.session_order.iter().copied().find_map(|session_id| {
            let entity = self.session_entity(session_id)?;
            self.get::<Uid>(entity)
                .is_some_and(|session_uid| session_uid.0 == uid)
                .then_some((session_id, entity))
        })
    }

    pub fn session_entity_by_object(
        &self,
        object: &session_core_session::SessionCoreSession,
    ) -> Option<(session::SessionId, Entity)> {
        self.session_entities
            .iter()
            .find(|(_, entity)| {
                self.get::<SessionObjects>(**entity).is_some_and(|objects| {
                    objects
                        .0
                        .iter()
                        .any(|session_object| session_object == object)
                })
            })
            .map(|(session_id, entity)| (*session_id, *entity))
    }

    pub fn insert<C>(&mut self, entity: Entity, component: C)
    where
        C: Component,
    {
        let storage = C::storage(self);
        if storage.len() <= entity.0 {
            storage.reserve(entity.0 + 1 - storage.len());
            storage.resize_with(entity.0 + 1, || None);
        }
        storage[entity.0] = Some(component);
    }

    pub fn get<C>(&self, entity: Entity) -> Option<&C>
    where
        C: Component,
    {
        C::storage_ref(self).get(entity.0)?.as_ref()
    }

    pub fn get_mut<C>(&mut self, entity: Entity) -> Option<&mut C>
    where
        C: Component,
    {
        C::storage(self).get_mut(entity.0)?.as_mut()
    }

    pub fn remove_entity(&mut self, entity: Entity) {
        clear_component(&mut self.uid_components, entity);
        clear_component(&mut self.username_components, entity);
        clear_component(&mut self.vt_components, entity);
        clear_component(&mut self.seat_components, entity);
        clear_component(&mut self.timestamp_components, entity);
        clear_component(&mut self.leader_id_components, entity);
        clear_component(&mut self.session_objects_components, entity);
        clear_component(&mut self.active_state_components, entity);
        self.free_entities.push(entity);
    }

    pub fn remove_session(&mut self, session_id: session::SessionId) -> Option<Entity> {
        let entity = self.session_entities.remove(&session_id)?;
        self.session_order
            .retain(|existing| *existing != session_id);
        self.remove_entity(entity);
        Some(entity)
    }
}

fn clear_component<C>(storage: &mut [Option<C>], entity: Entity) {
    if let Some(component) = storage.get_mut(entity.0) {
        *component = None;
    }
}
