use crate::{device, seat, seat_v1, session, session_core_session, session_core_v1};
use std::os::fd;
use std::{path, time};

pub trait Component: Sized {
    fn storage(world: &mut super::World) -> &mut Vec<Option<Self>>;
    fn storage_ref(world: &super::World) -> &Vec<Option<Self>>;
}

macro_rules! component {
    ($name:ident, $field:ident) => {
        impl Component for $name {
            fn storage(world: &mut super::World) -> &mut Vec<Option<Self>> {
                &mut world.$field
            }
            fn storage_ref(world: &super::World) -> &Vec<Option<Self>> {
                &world.$field
            }
        }
    };
}

pub struct Uid(pub u32);
component!(Uid, uid_components);

pub struct Username(pub String);
component!(Username, username_components);

pub struct Vt(pub u32);
component!(Vt, vt_components);

pub struct SeatName(pub String);
component!(SeatName, seat_name_components);

pub struct Timestamp(pub time::SystemTime);
component!(Timestamp, timestamp_components);

pub struct LeaderId(pub i32);
component!(LeaderId, leader_id_components);

pub struct SessionObjects(pub Vec<session_core_session::SessionCoreSession>);
component!(SessionObjects, session_objects_components);

pub struct ActiveState(pub session_core_v1::ActiveState);
component!(ActiveState, active_state_components);

pub struct Session(pub session::SessionId);
component!(Session, session_components);

pub struct SeatObject(pub seat_v1::seat::Seat);
component!(SeatObject, seat_object_components);

pub struct Devices(pub Vec<super::Entity>);
component!(Devices, devices_components);

pub struct DeviceObjects(pub Vec<seat_v1::device::Device>);
component!(DeviceObjects, device_objects_components);

pub struct DevicePath(pub path::PathBuf);
component!(DevicePath, device_path_components);

pub struct DeviceRefCount(pub usize);
component!(DeviceRefCount, device_ref_count_components);

pub struct DeviceKind(pub device::DeviceKind);
component!(DeviceKind, device_kind_components);

pub struct DeviceId(pub u32);
component!(DeviceId, device_id_components);

pub struct DeviceActive(pub bool);
component!(DeviceActive, device_active_components);

pub struct DeviceFd(pub fd::OwnedFd);
component!(DeviceFd, device_fd_components);

pub struct SeatState(pub seat::SeatState);
component!(SeatState, seat_state_components);

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SeatKind {
    VtBound,
    NonVtBound,
}

pub struct SeatMode(pub SeatKind);
component!(SeatMode, seat_mode_components);

pub struct CurrentVt(pub u32);
component!(CurrentVt, current_vt_components);

pub struct SeatClients(pub Vec<super::Entity>);
component!(SeatClients, seat_clients_components);

pub struct ActiveClient(pub super::Entity);
component!(ActiveClient, active_client_components);

pub struct NextClient(pub super::Entity);
component!(NextClient, next_client_components);
