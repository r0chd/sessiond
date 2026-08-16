use crate::{session, session_core_v1, terminal};
use bevy_ecs::prelude::*;
use std::num;

pub fn current_vt() -> anyhow::Result<num::NonZeroU32> {
    let tty0 = terminal::Terminal::open(0)?;
    num::NonZeroU32::new(u32::from(tty0.current_vt()?))
        .ok_or_else(|| anyhow::anyhow!("active VT is zero"))
}

pub fn refresh_current_vt(
    mut sessions: Query<
        (
            &session::Vt,
            Option<&session::IsLocal>,
            &mut session::ActiveState,
        ),
        With<session::Session>,
    >,
) {
    let Ok(current_vt) = current_vt() else {
        return;
    };

    for (vt, is_local, mut active_state) in &mut sessions {
        let desired = match (is_local, vt.0 == current_vt) {
            (Some(_), true) => session_core_v1::ActiveState::Active,
            (Some(_), false) => session_core_v1::ActiveState::Online,
            _ => continue,
        };

        if active_state.0 != desired {
            active_state.0 = desired;
        }
    }
}
