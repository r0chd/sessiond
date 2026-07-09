use crate::ecs;
use crate::session::{self, SessionId};

fn make_session_entity(world: &mut ecs::World, session_id: u32, uid: u32) -> ecs::Entity {
    let e = world.entity();
    world.insert(e, ecs::Session(SessionId::from_raw(session_id)));
    world.insert(e, ecs::Uid(uid));
    e
}

#[test]
fn session_id_round_trip() {
    let id = SessionId::from_raw(42);
    assert_eq!(id.as_raw(), 42);
}

#[test]
fn session_id_next() {
    let id = SessionId::from_raw(1);
    assert_eq!(id.next().as_raw(), 2);
}

#[test]
fn session_id_next_wraps_at_max() {
    let id = SessionId::from_raw(u32::MAX);
    assert_eq!(id.next().as_raw(), 0);
}

#[test]
fn entities_returns_only_session_entities() {
    let mut world = ecs::World::new();
    let with_session = make_session_entity(&mut world, 1, 1000);
    let without_session = world.entity();
    let all = vec![with_session, without_session];
    let result: Vec<_> = session::entities(&world, &all).collect();
    assert_eq!(result, vec![with_session]);
}

#[test]
fn entities_empty_when_none_have_session() {
    let mut world = ecs::World::new();
    let e = world.entity();
    let result: Vec<_> = session::entities(&world, &[e]).collect();
    assert!(result.is_empty());
}

#[test]
fn id_returns_session_id() {
    let mut world = ecs::World::new();
    let e = make_session_entity(&mut world, 42, 1000);
    assert_eq!(session::id(&world, e), Some(SessionId::from_raw(42)));
}

#[test]
fn id_returns_none_without_session_component() {
    let mut world = ecs::World::new();
    let e = world.entity();
    assert_eq!(session::id(&world, e), None);
}

#[test]
fn entity_finds_by_session_id() {
    let mut world = ecs::World::new();
    let e = make_session_entity(&mut world, 7, 1000);
    let all = vec![e];
    assert_eq!(
        session::entity(&world, &all, SessionId::from_raw(7)),
        Some(e)
    );
}

#[test]
fn entity_returns_none_for_unknown_id() {
    let mut world = ecs::World::new();
    let e = make_session_entity(&mut world, 1, 1000);
    let all = vec![e];
    assert!(session::entity(&world, &all, SessionId::from_raw(99)).is_none());
}

#[test]
fn entity_by_uid_finds_matching() {
    let mut world = ecs::World::new();
    let e = make_session_entity(&mut world, 1, 1000);
    let all = vec![e];
    assert_eq!(session::entity_by_uid(&world, &all, 1000), Some(e));
}

#[test]
fn entity_by_uid_returns_none_for_no_match() {
    let mut world = ecs::World::new();
    let e = make_session_entity(&mut world, 1, 1000);
    let all = vec![e];
    assert!(session::entity_by_uid(&world, &all, 9999).is_none());
}
