use crate::{device, seat, seat_v1, session, session_core_session, session_core_v1};
use bevy_ecs::prelude::*;
use std::os::fd;
use std::{path, time};

#[derive(Component)]
pub struct Uid(pub u32);

#[derive(Component)]
pub struct Username(pub String);

#[derive(Component)]
pub struct Vt(pub u32);

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
pub struct RemoteHost(pub String);

#[derive(Component)]
pub struct SessionObjects(pub Vec<session_core_session::SessionCoreSession>);

#[derive(Component)]
pub struct ActiveState(pub session_core_v1::ActiveState);

#[derive(Component)]
pub struct Session(pub session::SessionId);

#[derive(Component)]
pub struct SeatObject(pub seat_v1::seat::Seat);

#[derive(Component)]
pub struct Devices(pub Vec<Entity>);

#[derive(Component)]
pub struct DeviceObjects(pub Vec<seat_v1::device::Device>);

#[derive(Component)]
pub struct DevicePath(pub path::PathBuf);

#[derive(Component)]
pub struct DeviceRefCount(pub usize);

#[derive(Component)]
pub struct DeviceKind(pub device::DeviceKind);

#[derive(Component)]
pub struct DeviceId(pub u32);

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
pub struct ActiveClient(pub Entity);

#[derive(Component)]
pub struct NextClient(pub Entity);
