use crate::ecs;
use crate::session::{self, SessionId};
use crate::session_core_v1;
use bevy_ecs::prelude::{Entity, World};

fn make_session_entity(world: &mut World, session_id: u32, uid: u32) -> Entity {
    world
        .spawn((ecs::Session(SessionId::from_raw(session_id)), ecs::Uid(uid)))
        .id()
}

fn make_active_session_entity(
    world: &mut World,
    session_id: u32,
    uid: u32,
    state: session_core_v1::ActiveState,
) -> Entity {
    world
        .spawn((
            ecs::Session(SessionId::from_raw(session_id)),
            ecs::Uid(uid),
            ecs::ActiveState(state),
        ))
        .id()
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
    let mut world = World::new();
    let with_session = make_session_entity(&mut world, 1, 1000);
    let without_session = world.spawn_empty().id();
    let all = vec![with_session, without_session];
    let result: Vec<_> = session::entities(&world, &all).collect();
    assert_eq!(result, vec![with_session]);
}

#[test]
fn entities_empty_when_none_have_session() {
    let mut world = World::new();
    let e = world.spawn_empty().id();
    let result: Vec<_> = session::entities(&world, &[e]).collect();
    assert!(result.is_empty());
}

#[test]
fn id_returns_session_id() {
    let mut world = World::new();
    let e = make_session_entity(&mut world, 42, 1000);
    assert_eq!(session::id(&world, e), Some(SessionId::from_raw(42)));
}

#[test]
fn id_returns_none_without_session_component() {
    let mut world = World::new();
    let e = world.spawn_empty().id();
    assert_eq!(session::id(&world, e), None);
}

#[test]
fn entity_finds_by_session_id() {
    let mut world = World::new();
    let e = make_session_entity(&mut world, 7, 1000);
    let all = vec![e];
    assert_eq!(
        session::entity(&world, &all, SessionId::from_raw(7)),
        Some(e)
    );
}

#[test]
fn entity_returns_none_for_unknown_id() {
    let mut world = World::new();
    let e = make_session_entity(&mut world, 1, 1000);
    let all = vec![e];
    assert!(session::entity(&world, &all, SessionId::from_raw(99)).is_none());
}

#[test]
fn entity_by_uid_finds_matching() {
    let mut world = World::new();
    let e = make_session_entity(&mut world, 1, 1000);
    let all = vec![e];
    assert_eq!(session::entity_by_uid(&world, &all, 1000), Some(e));
}

#[test]
fn entity_by_uid_returns_none_for_no_match() {
    let mut world = World::new();
    let e = make_session_entity(&mut world, 1, 1000);
    let all = vec![e];
    assert!(session::entity_by_uid(&world, &all, 9999).is_none());
}

#[test]
fn entity_by_uid_skips_non_session_entities() {
    let mut world = World::new();
    let non_session = world.spawn(ecs::Uid(1000)).id();
    let session = make_session_entity(&mut world, 1, 1000);
    let all = vec![non_session, session];
    assert_eq!(session::entity_by_uid(&world, &all, 1000), Some(session));
}

#[test]
fn entity_by_uid_multiple_sessions_finds_correct() {
    let mut world = World::new();
    let e1 = make_session_entity(&mut world, 1, 1000);
    let e2 = make_session_entity(&mut world, 2, 2000);
    let all = vec![e1, e2];
    assert_eq!(session::entity_by_uid(&world, &all, 2000), Some(e2));
    assert_eq!(session::entity_by_uid(&world, &all, 1000), Some(e1));
}

#[test]
fn entities_returns_multiple_session_entities() {
    let mut world = World::new();
    let e1 = make_session_entity(&mut world, 1, 1000);
    let e2 = make_session_entity(&mut world, 2, 2000);
    let non = world.spawn_empty().id();
    let all = vec![e1, non, e2];
    let result: Vec<_> = session::entities(&world, &all).collect();
    assert_eq!(result, vec![e1, e2]);
}

#[test]
fn entity_finds_correct_among_multiple() {
    let mut world = World::new();
    let e1 = make_session_entity(&mut world, 1, 1000);
    let e2 = make_session_entity(&mut world, 2, 2000);
    let all = vec![e1, e2];
    assert_eq!(
        session::entity(&world, &all, SessionId::from_raw(2)),
        Some(e2)
    );
}

#[test]
fn set_session_state_updates_active_state() {
    let mut world = World::new();
    let e = make_active_session_entity(&mut world, 1, 1000, session_core_v1::ActiveState::Online);
    session::set_session_state(
        &mut world,
        SessionId::from_raw(1),
        e,
        session_core_v1::ActiveState::Active,
    );
    assert!(matches!(
        world.get::<ecs::ActiveState>(e).unwrap().0,
        session_core_v1::ActiveState::Active
    ));
}

#[test]
fn set_session_state_to_closing() {
    let mut world = World::new();
    let e = make_active_session_entity(&mut world, 1, 1000, session_core_v1::ActiveState::Active);
    session::set_session_state(
        &mut world,
        SessionId::from_raw(1),
        e,
        session_core_v1::ActiveState::Closing,
    );
    assert!(matches!(
        world.get::<ecs::ActiveState>(e).unwrap().0,
        session_core_v1::ActiveState::Closing
    ));
}

#[test]
fn set_session_state_without_active_state_component() {
    let mut world = World::new();
    let e = make_session_entity(&mut world, 1, 1000);
    session::set_session_state(
        &mut world,
        SessionId::from_raw(1),
        e,
        session_core_v1::ActiveState::Active,
    );
}

#[test]
fn set_session_state_with_vt_component() {
    let mut world = World::new();
    let e = make_active_session_entity(&mut world, 1, 1000, session_core_v1::ActiveState::Online);
    world.entity_mut(e).insert(ecs::Vt(Some(3)));
    session::set_session_state(
        &mut world,
        SessionId::from_raw(1),
        e,
        session_core_v1::ActiveState::Active,
    );
    assert!(matches!(
        world.get::<ecs::ActiveState>(e).unwrap().0,
        session_core_v1::ActiveState::Active
    ));
}

#[test]
fn set_state_updates_active_state() {
    let mut world = World::new();
    let e = make_active_session_entity(&mut world, 1, 1000, session_core_v1::ActiveState::Online);
    session::set_state(
        &mut world,
        SessionId::from_raw(1),
        e,
        session_core_v1::ActiveState::Active,
    );
    assert!(matches!(
        world.get::<ecs::ActiveState>(e).unwrap().0,
        session_core_v1::ActiveState::Active
    ));
}

#[test]
fn set_state_without_active_state_component() {
    let mut world = World::new();
    let e = make_session_entity(&mut world, 1, 1000);
    session::set_state(
        &mut world,
        SessionId::from_raw(1),
        e,
        session_core_v1::ActiveState::Closing,
    );
}

#[test]
fn set_state_with_vt_component() {
    let mut world = World::new();
    let e = make_active_session_entity(&mut world, 2, 2000, session_core_v1::ActiveState::Opening);
    world.entity_mut(e).insert(ecs::Vt(Some(7)));
    session::set_state(
        &mut world,
        SessionId::from_raw(2),
        e,
        session_core_v1::ActiveState::Online,
    );
    assert!(matches!(
        world.get::<ecs::ActiveState>(e).unwrap().0,
        session_core_v1::ActiveState::Online
    ));
}
