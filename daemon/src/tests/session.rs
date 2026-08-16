use crate::{session, session_core_v1};
use bevy_ecs::prelude::*;

fn make_session(world: &mut World, id: u32, state: session_core_v1::ActiveState) -> Entity {
    session::spawn(
        world,
        session::SessionId::from_raw(id),
        session::SessionClass::User,
        1000,
        "user".to_string(),
        Some(1),
        1234,
        None,
        state,
        None,
    )
}

#[test]
fn active_state_updates() {
    let mut world = World::new();
    let entity = make_session(&mut world, 1, session_core_v1::ActiveState::Online);
    session::set_active_state(&mut world, entity, session_core_v1::ActiveState::Active);
    assert_eq!(
        world.get::<session::ActiveState>(entity).unwrap().0,
        session_core_v1::ActiveState::Active
    );
}

#[test]
fn setting_the_same_active_state_is_not_a_change() {
    let mut world = World::new();
    let entity = make_session(&mut world, 1, session_core_v1::ActiveState::Online);

    world.clear_trackers();
    session::set_active_state(&mut world, entity, session_core_v1::ActiveState::Online);

    let mut query = world.query_filtered::<Entity, Changed<session::ActiveState>>();
    assert!(query.iter(&world).next().is_none());
}

#[test]
fn sessions_are_not_seat_bound() {
    let mut world = World::new();
    let entity = make_session(&mut world, 1, session_core_v1::ActiveState::Online);
    assert!(world.get::<session::SessionIdComponent>(entity).is_some());
}
