use crate::{session, terminal};
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
            &mut session::SessionState,
        ),
        With<session::Session>,
    >,
) {
    let Ok(current_vt) = current_vt() else {
        return;
    };

    for (vt, is_local, mut state) in &mut sessions {
        if *state == session::SessionState::CLOSING {
            continue;
        }

        let desired = match (is_local, vt.0 == current_vt) {
            (Some(_), true) => session::SessionState::ACTIVE,
            (Some(_), false) => session::SessionState::ONLINE,
            _ => continue,
        };

        if *state != desired {
            *state = desired;
        }
    }
}
