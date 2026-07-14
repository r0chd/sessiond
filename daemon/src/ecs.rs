use crate::session_core_v1::{
    session_core_seat, session_core_seat_client, session_core_seat_device,
};
use crate::session_management_v1::session_management_session;
use crate::{
    device, seat, seat_v1, session, session_core_session, session_core_user, session_core_v1,
};
use bevy_ecs::prelude::*;
use std::os::fd;
use std::{path, time};

#[allow(clippy::too_many_arguments)]
pub fn insert_session(
    world: &mut World,
    session_id: session::SessionId,
    uid: u32,
    username: String,
    vt: Option<u32>,
    seat: Option<String>,
    seat_entity: Option<Entity>,
    leader_id: i32,
    remote_host: Option<String>,
    active_state: session_core_v1::ActiveState,
    session_object: Option<session_management_session::SessionManagementSession>,
) -> Entity {
    let mut cmd = world.spawn((
        Session(session_id),
        Uid(uid),
        Username(username),
        Vt(vt),
        Timestamp(time::SystemTime::now()),
        LeaderId(leader_id),
        IsLocal(remote_host.is_none()),
        RemoteHost(remote_host),
        SessionObjects(Vec::new()),
        ActiveState(active_state),
    ));
    if let Some(name) = seat {
        cmd.insert(SeatName(name));
    }
    if let Some(entity) = seat_entity {
        cmd.insert(SeatEntity(entity));
    }
    if let Some(session_object) = session_object {
        cmd.insert(SessionObject(session_object));
    }
    cmd.id()
}

pub fn insert_user(
    world: &mut World,
    uid: u32,
    username: String,
    lingering: bool,
    state: session_core_v1::UserState,
) -> Entity {
    world
        .spawn((
            Uid(uid),
            Username(username),
            Lingering(lingering),
            UserState(state),
            UserObjects(Vec::new()),
        ))
        .id()
}

pub fn insert_client(world: &mut World, object: seat_v1::seat::Seat, pid: u32) -> Entity {
    world
        .spawn((
            SeatObject(object),
            ClientPid(pid),
            ClientObjects(Vec::new()),
            Devices(Vec::new()),
            SeatState(seat::SeatState::New),
        ))
        .id()
}

pub fn insert_seat(world: &mut World, name: String, mode: SeatKind, current_vt: u32) -> Entity {
    world
        .spawn((
            SeatName(name),
            SeatMode(mode),
            CurrentVt(current_vt),
            SeatClients(Vec::new()),
            SeatObjects(Vec::new()),
        ))
        .id()
}

pub fn insert_device(
    world: &mut World,
    object: seat_v1::device::Device,
    path: path::PathBuf,
    kind: device::DeviceKind,
    id: u32,
    active: bool,
    fd: fd::OwnedFd,
) -> Entity {
    world
        .spawn((
            DeviceObjects(vec![DeviceObject { object, id }]),
            SeatDeviceObjects(Vec::new()),
            DevicePath(path),
            DeviceRefCount(1),
            DeviceKind(kind),
            DeviceActive(active),
            DeviceFd(fd),
        ))
        .id()
}

#[derive(Component)]
pub struct Uid(pub u32);

#[derive(Component)]
pub struct Username(pub String);

#[derive(Component)]
pub struct Lingering(pub bool);

#[derive(Component)]
pub struct UserState(pub session_core_v1::UserState);

#[derive(Component)]
pub struct UserObjects(pub Vec<session_core_user::SessionCoreUser>);

#[derive(Component)]
pub struct Vt(pub Option<u32>);

#[derive(Component)]
pub struct SeatName(pub String);

#[derive(Component)]
pub struct SeatEntity(pub Entity);

#[derive(Component)]
pub struct Timestamp(pub time::SystemTime);

#[derive(Component)]
pub struct LeaderId(pub i32);

#[derive(Component)]
pub struct IsLocal(pub bool);

#[derive(Component)]
pub struct RemoteHost(pub Option<String>);

#[derive(Component)]
pub struct SessionObjects(pub Vec<session_core_session::SessionCoreSession>);

#[derive(Component)]
pub struct ActiveState(pub session_core_v1::ActiveState);

#[derive(Component)]
pub struct Session(pub session::SessionId);

#[derive(Component)]
pub struct SeatObject(pub seat_v1::seat::Seat);

#[derive(Component)]
pub struct ClientPid(pub u32);

#[derive(Component)]
pub struct ClientObjects(pub Vec<session_core_seat_client::SessionCoreSeatClient>);

#[derive(Component)]
pub struct Devices(pub Vec<Entity>);

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

#[derive(Component)]
pub struct DeviceRefCount(pub usize);

#[derive(Component)]
pub struct DeviceKind(pub device::DeviceKind);

#[derive(Component)]
pub struct DeviceActive(pub bool);

#[derive(Component)]
pub struct DeviceFd(pub fd::OwnedFd);

#[derive(Component)]
pub struct SeatState(pub seat::SeatState);

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SeatKind {
    VtBound,
    NonVtBound,
}

#[derive(Component)]
pub struct SeatMode(pub SeatKind);

#[derive(Component)]
pub struct CurrentVt(pub u32);

#[derive(Component)]
pub struct SeatClients(pub Vec<Entity>);

#[derive(Component)]
pub struct SeatObjects(pub Vec<session_core_seat::SessionCoreSeat>);

#[derive(Component)]
pub struct ActiveClient(pub Entity);

#[derive(Component)]
pub struct NextClient(pub Entity);

#[derive(Component)]
pub struct SessionObject(pub session_management_session::SessionManagementSession);
