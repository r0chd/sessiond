use crate::session_core_v1;
use crate::{ecs, seat, session};
use bevy_ecs::prelude::*;

#[test]
fn spawn_seat_sets_expected_components() {
    let mut world = World::new();
    let seat = seat::spawn(
        &mut world,
        "seat0".to_string(),
        ecs::SeatKind::NonVtBound,
        0,
    );
    assert_eq!(
        world.get::<ecs::SeatName>(seat).map(|n| n.0.as_str()),
        Some("seat0")
    );
    assert!(matches!(
        world.get::<ecs::SeatMode>(seat).map(|m| m.0),
        Some(ecs::SeatKind::NonVtBound)
    ));
    assert!(world.get::<ecs::CurrentVt>(seat).is_none());
}

#[test]
fn spawn_session_sets_expected_components() {
    let mut world = World::new();
    let id = session::SessionId::from_raw(1);
    let entity = session::spawn(
        &mut world,
        id,
        1000,
        "alice".to_string(),
        Some(2),
        Some("seat0".to_string()),
        None,
        1234,
        None,
        session_core_v1::ActiveState::Active,
        None,
    );
    assert_eq!(
        world.get::<ecs::SessionId>(entity).map(|s| s.0.as_raw()),
        Some(1)
    );
    assert_eq!(world.get::<ecs::Uid>(entity).map(|u| u.0), Some(1000));
    assert_eq!(world.get::<ecs::Vt>(entity).map(|v| v.0.get()), Some(2));
    assert_eq!(
        world.get::<ecs::ActiveState>(entity).map(|a| a.0),
        Some(session_core_v1::ActiveState::Active)
    );
}
