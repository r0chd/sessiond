use crate::session_core_v1::{
    session_core_seat, session_core_seat_client, session_core_seat_device,
};
use crate::session_management_v2::session_management_session;
use crate::{
    device, seat, seat_v1, session, session_core_session, session_core_user, session_core_v1,
};
use bevy_ecs::prelude::*;
use std::collections::BTreeMap;
use std::os::fd;
use std::{num, path, time};

// Bundles

#[derive(Bundle)]
pub struct SessionBundle {
    pub marker: Session,
    pub id: SessionId,
    pub uid: Uid,
    pub username: Username,
    pub timestamp: Timestamp,
    pub leader_id: LeaderId,
    pub env: SessionEnv,
    pub objects: SessionObjects,
    pub active_state: ActiveState,
}

#[derive(Bundle)]
pub struct UserBundle {
    pub marker: User,
    pub uid: Uid,
    pub username: Username,
    pub state: UserState,
    pub objects: UserObjects,
}

#[derive(Bundle)]
pub struct ClientBundle {
    pub marker: Client,
    pub seat_object: SeatObject,
    pub pid: ClientPid,
    pub objects: ClientObjects,
    pub state: SeatState,
}

#[derive(Bundle)]
pub struct SeatBundle {
    pub marker: Seat,
    pub name: SeatName,
    pub mode: SeatMode,
    pub objects: SeatObjects,
}

#[derive(Bundle)]
pub struct DeviceBundle {
    pub marker: Device,
    pub objects: DeviceObjects,
    pub seat_objects: SeatDeviceObjects,
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

#[derive(Component)]
pub struct UserObjects(pub Vec<session_core_user::SessionCoreUser>);

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

#[derive(Component)]
pub struct SessionObjects(pub Vec<session_core_session::SessionCoreSession>);

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

#[derive(Component)]
pub struct SeatObjects(pub Vec<session_core_seat::SessionCoreSeat>);

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

#[derive(Component)]
pub struct ClientObjects(pub Vec<session_core_seat_client::SessionCoreSeatClient>);

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClientSeat(pub Entity);

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct SeatState(pub seat::SeatState);

// Device components

#[derive(Component)]
pub struct DeviceObjects(pub Vec<DeviceObject>);

#[derive(Component)]
pub struct SeatDeviceObjects(pub Vec<SeatDeviceObject>);

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
