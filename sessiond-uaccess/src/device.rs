use crate::ecs;
use bevy_ecs::prelude::*;
use std::path;

pub fn entity_by_syspath(world: &mut World, syspath: &path::Path) -> Option<Entity> {
    let mut devices = world.query_filtered::<(Entity, &ecs::Syspath), With<ecs::Device>>();
    devices
        .iter(world)
        .find_map(|(entity, device_syspath)| (device_syspath.0 == syspath).then_some(entity))
}
