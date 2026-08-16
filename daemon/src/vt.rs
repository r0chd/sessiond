use crate::{session, session_core_v1, terminal};
use bevy_ecs::prelude::*;
use std::num;

pub fn current_vt() -> anyhow::Result<num::NonZeroU32> {
    let tty0 = terminal::Terminal::open(0)?;
    num::NonZeroU32::new(u32::from(tty0.current_vt()?))
        .ok_or_else(|| anyhow::anyhow!("active VT is zero"))
}

pub fn refresh_current_vt(world: &mut World) {
    let Ok(current) = current_vt() else { return };
    for entity in session::entities(world) {
        let state = match (
            world.get::<session::Vt>(entity),
            world.get::<session::IsLocal>(entity),
        ) {
            (Some(vt), Some(_)) if vt.0 == current => session_core_v1::ActiveState::Active,
            (Some(_), Some(_)) => session_core_v1::ActiveState::Online,
            _ => continue,
        };
        session::set_active_state(world, entity, state);
    }
}

pub fn refresh_current_vts(world: &mut World) {
    refresh_current_vt(world);
}
