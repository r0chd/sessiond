use crate::session_core_v1::session_core_session;
use crate::{config, session_core_v1};
use bevy_ecs::prelude::*;
use std::{path, sync};

pub fn insert_session(
    world: &mut World,
    object: session_core_session::SessionCoreSession,
) -> Entity {
    world
        .spawn((Session, SessionObject(object), Done(false)))
        .id()
}

#[derive(Component)]
pub struct Syspath(pub sync::Arc<path::Path>);

#[derive(Component)]
pub struct Devnode(pub path::PathBuf);

#[derive(Component)]
pub struct Session;

#[derive(Component)]
pub struct SessionObject(pub session_core_session::SessionCoreSession);

#[derive(Component)]
pub struct DeviceEntity(pub Entity);

#[derive(Component)]
pub struct Uid(pub u32);

#[derive(Component)]
pub struct AccessMode(pub config::AccessMode);

#[derive(Component)]
pub struct ActiveState(pub session_core_v1::ActiveState);

#[derive(Component)]
pub struct IsLocal(pub bool);

#[derive(Component)]
pub struct Done(pub bool);

#[derive(Component)]
pub struct Device;

#[derive(Component)]
pub struct Grant;

#[derive(Message)]
pub struct DeviceRemoved {
    pub syspath: sync::Arc<path::Path>,
}

#[derive(Message)]
pub struct DeviceAdded {
    pub syspath: sync::Arc<path::Path>,
}
