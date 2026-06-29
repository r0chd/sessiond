use crate::{session_core_session, session_core_v1};
use std::time;

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

pub struct Seat(pub Option<String>);
component!(Seat, seat_components);

pub struct Timestamp(pub time::SystemTime);
component!(Timestamp, timestamp_components);

pub struct LeaderId(pub i32);
component!(LeaderId, leader_id_components);

pub struct SessionObjects(pub Vec<session_core_session::SessionCoreSession>);
component!(SessionObjects, session_objects_components);

pub struct ActiveState(pub session_core_v1::ActiveState);
component!(ActiveState, active_state_components);
