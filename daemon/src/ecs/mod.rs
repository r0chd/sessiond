mod components;

use crate::session;
use crate::{seat, seat_v1, session_core_session, session_core_v1};
pub use components::*;
use std::time;

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Entity(usize);

#[derive(Default)]
pub struct World {
    next_entity: usize,
    free_entities: Vec<Entity>,
    uid_components: Vec<Option<Uid>>,
    username_components: Vec<Option<Username>>,
    vt_components: Vec<Option<Vt>>,
    seat_name_components: Vec<Option<SeatName>>,
    timestamp_components: Vec<Option<Timestamp>>,
    leader_id_components: Vec<Option<LeaderId>>,
    session_objects_components: Vec<Option<SessionObjects>>,
    active_state_components: Vec<Option<ActiveState>>,
    session_components: Vec<Option<Session>>,
    seat_object_components: Vec<Option<SeatObject>>,
    devices_components: Vec<Option<Devices>>,
    seat_state_components: Vec<Option<SeatState>>,
    seat_mode_components: Vec<Option<SeatMode>>,
    current_vt_components: Vec<Option<CurrentVt>>,
    seat_clients_components: Vec<Option<SeatClients>>,
    active_client_components: Vec<Option<ActiveClient>>,
    next_client_components: Vec<Option<NextClient>>,
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
        self.insert(entity, Session(session_id));
        self.insert(entity, Uid(uid));
        self.insert(entity, Username(username));
        self.insert(entity, Vt(vt));
        if let Some(seat) = seat {
            self.insert(entity, SeatName(seat));
        }
        self.insert(entity, Timestamp(timestamp));
        self.insert(entity, LeaderId(leader_id));
        self.insert(entity, SessionObjects(objects));
        self.insert(entity, ActiveState(active_state));
        entity
    }

    pub fn insert_client(&mut self, object: seat_v1::seat::Seat) -> Entity {
        let entity = self.entity();
        self.insert(entity, SeatObject(object));
        self.insert(entity, Devices(Vec::new()));
        self.insert(entity, SeatState(seat::SeatState::New));
        entity
    }

    pub fn insert_seat(&mut self, name: String, mode: SeatKind, current_vt: u32) -> Entity {
        let entity = self.entity();
        self.insert(entity, SeatName(name));
        self.insert(entity, SeatMode(mode));
        self.insert(entity, CurrentVt(current_vt));
        self.insert(entity, SeatClients(Vec::new()));
        entity
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

    pub fn remove<C>(&mut self, entity: Entity) -> Option<C>
    where
        C: Component,
    {
        C::storage(self).get_mut(entity.0)?.take()
    }

    pub fn remove_entity(&mut self, entity: Entity) {
        clear_component(&mut self.uid_components, entity);
        clear_component(&mut self.username_components, entity);
        clear_component(&mut self.vt_components, entity);
        clear_component(&mut self.seat_name_components, entity);
        clear_component(&mut self.timestamp_components, entity);
        clear_component(&mut self.leader_id_components, entity);
        clear_component(&mut self.session_objects_components, entity);
        clear_component(&mut self.active_state_components, entity);
        clear_component(&mut self.session_components, entity);
        clear_component(&mut self.seat_object_components, entity);
        clear_component(&mut self.devices_components, entity);
        clear_component(&mut self.seat_state_components, entity);
        clear_component(&mut self.seat_mode_components, entity);
        clear_component(&mut self.current_vt_components, entity);
        clear_component(&mut self.seat_clients_components, entity);
        clear_component(&mut self.active_client_components, entity);
        clear_component(&mut self.next_client_components, entity);
        self.free_entities.push(entity);
    }
}

fn clear_component<C>(storage: &mut [Option<C>], entity: Entity) {
    if let Some(component) = storage.get_mut(entity.0) {
        *component = None;
    }
}
