mod components;

use crate::session;
pub use components::*;
use crate::{session_core_session, session_core_v1};
use std::collections::HashMap;
use std::time;

#[derive(Copy, Clone, Default)]
pub struct Entity(usize);

#[derive(Default)]
pub struct World {
    next_entity: usize,
    free_entities: Vec<Entity>,
    session_entities: HashMap<session::SessionId, Entity>,
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
        self.free_entities.push(entity);
    }
}
