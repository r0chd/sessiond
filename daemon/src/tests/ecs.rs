use crate::ecs::{self, SeatKind, Uid, SeatName, SeatMode, CurrentVt, SeatClients, Session, ActiveState};
use crate::session::SessionId;
use crate::session_core_v1;
use std::time;

#[test]
fn entity_allocation_is_sequential() {
    let mut world = ecs::World::new();
    let a = world.entity();
    let b = world.entity();
    assert_ne!(a, b);
}

#[test]
fn freed_entity_is_reused() {
    let mut world = ecs::World::new();
    let a = world.entity();
    world.insert(a, Uid(1));
    world.remove_entity(a);
    let b = world.entity();
    assert_eq!(a, b);
}

#[test]
fn component_insert_and_get() {
    let mut world = ecs::World::new();
    let e = world.entity();
    world.insert(e, Uid(42));
    assert_eq!(world.get::<Uid>(e).map(|u| u.0), Some(42));
}

#[test]
fn component_overwrite() {
    let mut world = ecs::World::new();
    let e = world.entity();
    world.insert(e, Uid(1));
    world.insert(e, Uid(2));
    assert_eq!(world.get::<Uid>(e).map(|u| u.0), Some(2));
}

#[test]
fn component_absent_returns_none() {
    let mut world = ecs::World::new();
    let e = world.entity();
    assert!(world.get::<Uid>(e).is_none());
}

#[test]
fn component_get_mut() {
    let mut world = ecs::World::new();
    let e = world.entity();
    world.insert(e, Uid(1));
    world.get_mut::<Uid>(e).unwrap().0 = 99;
    assert_eq!(world.get::<Uid>(e).map(|u| u.0), Some(99));
}

#[test]
fn component_remove_returns_value() {
    let mut world = ecs::World::new();
    let e = world.entity();
    world.insert(e, Uid(7));
    let removed = world.remove::<Uid>(e);
    assert_eq!(removed.map(|u| u.0), Some(7));
    assert!(world.get::<Uid>(e).is_none());
}

#[test]
fn remove_entity_clears_all_components() {
    let mut world = ecs::World::new();
    let e = world.entity();
    world.insert(e, Uid(1));
    world.insert(e, SeatName("seat0".to_string()));
    world.remove_entity(e);
    assert!(world.get::<Uid>(e).is_none());
    assert!(world.get::<SeatName>(e).is_none());
}

#[test]
fn insert_seat_sets_expected_components() {
    let mut world = ecs::World::new();
    let seat = world.insert_seat("seat0".to_string(), SeatKind::NonVtBound, 0);
    assert_eq!(world.get::<SeatName>(seat).map(|n| n.0.as_str()), Some("seat0"));
    assert_eq!(world.get::<SeatMode>(seat).map(|m| m.0), Some(SeatKind::NonVtBound));
    assert_eq!(world.get::<CurrentVt>(seat).map(|v| v.0), Some(0));
    assert!(world.get::<SeatClients>(seat).is_some());
}

#[test]
fn insert_session_sets_expected_components() {
    let mut world = ecs::World::new();
    let id = SessionId::from_raw(1);
    let ts = time::UNIX_EPOCH;
    let entity = world.insert_session(
        id, 1000, "alice".to_string(), 2, Some("seat0".to_string()),
        None, ts, 1234, true, String::new(), vec![],
        session_core_v1::ActiveState::Active,
    );
    assert_eq!(world.get::<Session>(entity).map(|s| s.0.as_raw()), Some(1));
    assert_eq!(world.get::<ecs::Uid>(entity).map(|u| u.0), Some(1000));
    assert_eq!(world.get::<ecs::Vt>(entity).map(|v| v.0), Some(2));
    assert_eq!(world.get::<ActiveState>(entity).map(|a| a.0), Some(session_core_v1::ActiveState::Active));
}
