use crate::session_core_v1::{
    session_core_manager, session_core_seat, session_core_seat_client, session_core_seat_device,
};
use crate::session_management_v1::session_manager;
use crate::session_management_v2::session_management_session;
use crate::{
    config, device, seat, seat_v1, session, session_core_session, session_core_user,
    session_core_v1, session_management_v1, session_management_v2,
};
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::ScheduleLabel;
use std::collections::{BTreeMap, HashMap};
use std::os::fd;
use std::{num, path, time};

// Resources

#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Notify;

#[derive(Resource)]
pub struct AccessRules {
    pub seat: crate::Access,
    pub management: crate::Access,
}

#[derive(Resource)]
pub struct DaemonConfig(pub config::Config);

#[derive(Resource, Default)]
pub struct SessionInfoManagers(pub Vec<session_core_manager::SessionCoreManager>);

#[derive(Resource, Default)]
pub struct NotifyBatch {
    pub sessions: Vec<Entity>,
    pub users: Vec<Entity>,
    pub clients: Vec<Entity>,
    pub devices: Vec<Entity>,
}

impl NotifyBatch {
    pub fn session(&mut self, entity: Entity) {
        if !self.sessions.contains(&entity) {
            self.sessions.push(entity);
        }
    }

    pub fn user(&mut self, entity: Entity) {
        if !self.users.contains(&entity) {
            self.users.push(entity);
        }
    }

    pub fn client(&mut self, entity: Entity) {
        if !self.clients.contains(&entity) {
            self.clients.push(entity);
        }
    }

    pub fn device(&mut self, entity: Entity) {
        if !self.devices.contains(&entity) {
            self.devices.push(entity);
        }
    }
}

#[derive(Resource, Default)]
pub struct SeatNotifyState {
    pub active_clients: HashMap<Entity, Entity>,
    pub next_clients: HashMap<Entity, Entity>,
}

#[derive(Resource, Default)]
pub struct PendingDeviceActiveNotifications(pub Vec<(Entity, bool)>);

#[derive(Resource)]
pub struct NextSessionId(pub session::SessionId);

impl NextSessionId {
    pub const fn take(&mut self) -> session::SessionId {
        let id = self.0;
        self.0 = self.0.next();
        id
    }
}

#[derive(Resource, Default)]
pub struct PendingSessionEnvs(
    pub HashMap<session_manager::SessionManager, BTreeMap<String, String>>,
);

#[derive(Resource, Default)]
pub struct ProtocolObjects {
    pub sessions: HashMap<Entity, Vec<session_core_session::SessionCoreSession>>,
    pub users: HashMap<Entity, Vec<session_core_user::SessionCoreUser>>,
    pub seats: HashMap<Entity, Vec<session_core_seat::SessionCoreSeat>>,
    pub clients: HashMap<Entity, Vec<session_core_seat_client::SessionCoreSeatClient>>,
    pub devices: HashMap<Entity, Vec<DeviceObject>>,
    pub seat_devices: HashMap<Entity, Vec<SeatDeviceObject>>,
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

    pub fn seat_objects(&self, entity: Entity) -> Option<&[session_core_seat::SessionCoreSeat]> {
        self.seats.get(&entity).map(Vec::as_slice)
    }

    pub fn add_seat_object(&mut self, entity: Entity, object: session_core_seat::SessionCoreSeat) {
        self.seats.entry(entity).or_default().push(object);
    }

    pub fn remove_seat_object(
        &mut self,
        entity: Entity,
        object: &session_core_seat::SessionCoreSeat,
    ) {
        if let Some(objects) = self.seats.get_mut(&entity) {
            objects.retain(|candidate| candidate != object);
        }
    }

    pub fn seat_entity_by_object(
        &self,
        object: &session_core_seat::SessionCoreSeat,
    ) -> Option<Entity> {
        self.seats.iter().find_map(|(entity, objects)| {
            objects
                .iter()
                .any(|candidate| candidate == object)
                .then_some(*entity)
        })
    }

    pub fn client_objects(
        &self,
        entity: Entity,
    ) -> Option<&[session_core_seat_client::SessionCoreSeatClient]> {
        self.clients.get(&entity).map(Vec::as_slice)
    }

    pub fn add_client_object(
        &mut self,
        entity: Entity,
        object: session_core_seat_client::SessionCoreSeatClient,
    ) {
        self.clients.entry(entity).or_default().push(object);
    }

    pub fn remove_client_object(
        &mut self,
        entity: Entity,
        object: &session_core_seat_client::SessionCoreSeatClient,
    ) {
        if let Some(objects) = self.clients.get_mut(&entity) {
            objects.retain(|candidate| candidate != object);
        }
    }

    pub fn remove_client_objects(&mut self, entity: Entity) {
        self.clients.remove(&entity);
    }

    pub fn client_entity_by_object(
        &self,
        object: &session_core_seat_client::SessionCoreSeatClient,
    ) -> Option<Entity> {
        self.clients.iter().find_map(|(entity, objects)| {
            objects
                .iter()
                .any(|candidate| candidate == object)
                .then_some(*entity)
        })
    }

    pub fn device_objects(&self, entity: Entity) -> Option<&[DeviceObject]> {
        self.devices.get(&entity).map(Vec::as_slice)
    }

    pub fn add_device_object(&mut self, entity: Entity, object: DeviceObject) {
        self.devices.entry(entity).or_default().push(object);
    }

    pub fn remove_device_object(&mut self, entity: Entity, object: &seat_v1::device::Device) {
        if let Some(objects) = self.devices.get_mut(&entity) {
            objects.retain(|candidate| candidate.object != *object);
        }
    }

    pub fn remove_device_objects(&mut self, entity: Entity) {
        self.devices.remove(&entity);
        self.seat_devices.remove(&entity);
    }

    pub fn seat_device_objects(&self, entity: Entity) -> Option<&[SeatDeviceObject]> {
        self.seat_devices.get(&entity).map(Vec::as_slice)
    }

    pub fn add_seat_device_object(&mut self, entity: Entity, object: SeatDeviceObject) {
        self.seat_devices.entry(entity).or_default().push(object);
    }

    pub fn remove_seat_device_object(
        &mut self,
        entity: Entity,
        object: &session_core_seat_device::SessionCoreSeatDevice,
    ) {
        if let Some(objects) = self.seat_devices.get_mut(&entity) {
            objects.retain(|candidate| candidate.object != *object);
        }
    }

    pub fn remove_seat_device_objects_by_id(&mut self, entity: Entity, id: u32) {
        if let Some(objects) = self.seat_devices.get_mut(&entity) {
            objects.retain(|candidate| candidate.id != id);
        }
    }
}

pub fn init_resources(world: &mut World) {
    world.insert_resource(ProtocolObjects::default());
    world.insert_resource(NotifyBatch::default());
    world.insert_resource(SeatNotifyState::default());
    world.insert_resource(PendingDeviceActiveNotifications::default());
}

pub fn push_device_active_notification(world: &mut World, entity: Entity, active: bool) {
    world
        .resource_mut::<PendingDeviceActiveNotifications>()
        .0
        .push((entity, active));
}

// Bundles

#[derive(Bundle)]
pub struct SessionBundle {
    pub marker: Session,
    pub id: SessionId,
    pub class: SessionClass,
    pub uid: Uid,
    pub username: Username,
    pub timestamp: Timestamp,
    pub leader_id: LeaderId,
    pub env: SessionEnv,
    pub active_state: ActiveState,
}

#[derive(Bundle)]
pub struct UserBundle {
    pub marker: User,
    pub uid: Uid,
    pub username: Username,
    pub state: UserState,
}

#[derive(Bundle)]
pub struct ClientBundle {
    pub marker: Client,
    pub seat_object: SeatObject,
    pub pid: ClientPid,
    pub state: SeatState,
    pub session: ClientSession,
}

#[derive(Bundle)]
pub struct SeatBundle {
    pub marker: Seat,
    pub name: SeatName,
    pub mode: SeatMode,
}

#[derive(Bundle)]
pub struct DeviceBundle {
    pub marker: Device,
    pub path: DevicePath,
    pub ref_count: DeviceRefCount,
    pub kind: DeviceKind,
    pub fd: DeviceFd,
    pub owner: OwnerClient,
}

// Marker components

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Session;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Client;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct User;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Seat;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Device;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lingering;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct IsLocal;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceActive;

// Shared components

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Uid(pub u32);

#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct Username(pub String);

#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct SeatName(pub String);

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionId(pub session::SessionId);

// User components

#[derive(Component)]
pub struct UserState(pub session_core_v1::UserState);

// Session components

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Vt(pub num::NonZeroU32);

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionSeat(pub Entity);

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timestamp(pub time::SystemTime);

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct LeaderId(pub i32);

#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct RemoteHost(pub String);

#[derive(Component)]
pub struct SessionEnv(pub BTreeMap<String, String>);

#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum SessionClass {
    #[default]
    User,
    Greeter,
    LockScreen,
    Background,
    BackgroundLight,
    Manager,
    ManagerEarly,
}

impl From<session_management_v1::SessionClass> for SessionClass {
    fn from(class: session_management_v1::SessionClass) -> Self {
        match class {
            session_management_v1::SessionClass::User => Self::User,
            session_management_v1::SessionClass::Greeter => Self::Greeter,
            session_management_v1::SessionClass::LockScreen => Self::LockScreen,
            session_management_v1::SessionClass::Background => Self::Background,
            session_management_v1::SessionClass::BackgroundLight => Self::BackgroundLight,
            session_management_v1::SessionClass::Manager => Self::Manager,
            session_management_v1::SessionClass::ManagerEarly => Self::ManagerEarly,
        }
    }
}

impl From<session_management_v2::SessionClass> for SessionClass {
    fn from(class: session_management_v2::SessionClass) -> Self {
        match class {
            session_management_v2::SessionClass::User => Self::User,
            session_management_v2::SessionClass::Greeter => Self::Greeter,
            session_management_v2::SessionClass::LockScreen => Self::LockScreen,
            session_management_v2::SessionClass::Background => Self::Background,
            session_management_v2::SessionClass::BackgroundLight => Self::BackgroundLight,
            session_management_v2::SessionClass::Manager => Self::Manager,
            session_management_v2::SessionClass::ManagerEarly => Self::ManagerEarly,
        }
    }
}

impl From<SessionClass> for session_core_v1::SessionClass {
    fn from(class: SessionClass) -> Self {
        match class {
            SessionClass::User => Self::User,
            SessionClass::Greeter => Self::Greeter,
            SessionClass::LockScreen => Self::LockScreen,
            SessionClass::Background => Self::Background,
            SessionClass::BackgroundLight => Self::BackgroundLight,
            SessionClass::Manager => Self::Manager,
            SessionClass::ManagerEarly => Self::ManagerEarly,
        }
    }
}

#[derive(Component)]
pub struct ActiveState(pub session_core_v1::ActiveState);

#[derive(Component)]
pub struct SessionObject(pub session_management_session::SessionManagementSession);

// Seat components

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SeatKind {
    VtBound,
    NonVtBound,
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct SeatMode(pub SeatKind);

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct CurrentVt(pub num::NonZeroU32);

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActiveClient(pub Entity);

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct NextClient(pub Entity);

// Client components

#[derive(Component)]
pub struct SeatObject(pub seat_v1::seat::Seat);

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClientPid(pub u32);

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClientSession(pub session::SessionId);

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClientSeat(pub Entity);

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct SeatState(pub seat::SeatState);

// Device components

pub struct SeatDeviceObject {
    pub object: session_core_seat_device::SessionCoreSeatDevice,
    pub id: u32,
}

pub struct DeviceObject {
    pub object: seat_v1::device::Device,
    pub id: u32,
}

#[derive(Component)]
pub struct DevicePath(pub path::PathBuf);

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceRefCount(pub usize);

#[derive(Component)]
pub struct DeviceKind(pub device::DeviceKind);

#[derive(Component)]
pub struct DeviceFd(pub fd::OwnedFd);

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct OwnerClient(pub Entity);
