mod components;

use crate::session;
use crate::{device, seat, seat_v1, session_core_session, session_core_v1};
pub use bevy_ecs::entity::Entity;
pub use bevy_ecs::world::World;
pub use components::*;
use std::os::fd;
use std::path;
use std::time;

#[allow(clippy::too_many_arguments)]
pub fn insert_session(
    world: &mut World,
    session_id: session::SessionId,
    uid: u32,
    username: String,
    vt: u32,
    seat: Option<String>,
    seat_entity: Option<Entity>,
    timestamp: time::SystemTime,
    leader_id: i32,
    is_local: bool,
    remote_host: String,
    objects: Vec<session_core_session::SessionCoreSession>,
    active_state: session_core_v1::ActiveState,
) -> Entity {
    let mut cmd = world.spawn((
        Session(session_id),
        Uid(uid),
        Username(username),
        Vt(vt),
        Timestamp(timestamp),
        LeaderId(leader_id),
        IsLocal(is_local),
        RemoteHost(remote_host),
        SessionObjects(objects),
        ActiveState(active_state),
    ));
    if let Some(name) = seat {
        cmd.insert(SeatName(name));
    }
    if let Some(entity) = seat_entity {
        cmd.insert(SeatEntity(entity));
    }
    cmd.id()
}

pub fn insert_client(world: &mut World, object: seat_v1::seat::Seat) -> Entity {
    world
        .spawn((
            SeatObject(object),
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
            DeviceObjects(vec![object]),
            DevicePath(path),
            DeviceRefCount(1),
            DeviceKind(kind),
            DeviceId(id),
            DeviceActive(active),
            DeviceFd(fd),
        ))
        .id()
}
