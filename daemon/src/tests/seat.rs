use crate::{client, ecs, seat, session};
use bevy_ecs::prelude::*;

fn test_world() -> World {
    let mut world = World::new();
    ecs::init_resources(&mut world);
    world
}

fn make_seat(world: &mut World) -> Entity {
    seat::spawn(world, "seat0".to_string(), ecs::SeatKind::NonVtBound, 0)
}

fn make_client(world: &mut World) -> Entity {
    world
        .spawn((ecs::Client, ecs::SeatState(seat::SeatState::New)))
        .id()
}

fn make_client_with_pid(world: &mut World, pid: u32) -> Entity {
    world
        .spawn((
            ecs::Client,
            ecs::ClientPid(pid),
            ecs::SeatState(seat::SeatState::New),
        ))
        .id()
}

#[test]
fn add_assigns_session_id_1() {
    let mut world = test_world();
    let seat = make_seat(&mut world);
    let client = make_client(&mut world);
    client::add(&mut world, seat, client).unwrap();
    assert_eq!(
        world.get::<ecs::SessionId>(client).map(|s| s.0.as_raw()),
        Some(1)
    );
}

#[test]
fn add_assigns_sequential_ids() {
    let mut world = test_world();
    let seat = make_seat(&mut world);
    let c1 = make_client(&mut world);
    let c2 = make_client(&mut world);
    client::add(&mut world, seat, c1).unwrap();
    client::add(&mut world, seat, c2).unwrap();
    assert_eq!(
        world.get::<ecs::SessionId>(c1).map(|s| s.0.as_raw()),
        Some(1)
    );
    assert_eq!(
        world.get::<ecs::SessionId>(c2).map(|s| s.0.as_raw()),
        Some(2)
    );
}

#[test]
fn add_sets_enabled_state() {
    let mut world = test_world();
    let seat = make_seat(&mut world);
    let client = make_client(&mut world);
    client::add(&mut world, seat, client).unwrap();
    assert_eq!(
        world.get::<ecs::SeatState>(client).map(|s| s.0),
        Some(seat::SeatState::Enabled)
    );
}

#[test]
fn add_sets_active_client() {
    let mut world = test_world();
    let seat = make_seat(&mut world);
    let client = make_client(&mut world);
    client::add(&mut world, seat, client).unwrap();
    assert_eq!(
        world.get::<ecs::ActiveClients>(seat).map(|a| a.0.clone()),
        Some(vec![client])
    );
}

#[test]
fn add_keeps_multiple_clients_active() {
    let mut world = test_world();
    let seat = make_seat(&mut world);
    let c1 = make_client_with_pid(&mut world, 100);
    let c2 = make_client_with_pid(&mut world, 200);
    client::add(&mut world, seat, c1).unwrap();
    client::add(&mut world, seat, c2).unwrap();
    assert_eq!(
        world
            .get::<ecs::ActiveClients>(seat)
            .map(|clients| clients.0.clone()),
        Some(vec![c1, c2])
    );
}

#[test]
fn add_rejects_duplicate_pid() {
    let mut world = test_world();
    let seat = make_seat(&mut world);
    let c1 = make_client_with_pid(&mut world, 100);
    let c2 = make_client_with_pid(&mut world, 100);
    client::add(&mut world, seat, c1).unwrap();
    assert!(client::add(&mut world, seat, c2).is_err());
}

#[test]
fn removing_one_active_client_preserves_the_others() {
    let mut world = test_world();
    let seat = make_seat(&mut world);
    let c1 = make_client_with_pid(&mut world, 100);
    let c2 = make_client_with_pid(&mut world, 200);
    client::add(&mut world, seat, c1).unwrap();
    client::add(&mut world, seat, c2).unwrap();
    seat::remove_client(&mut world, seat, c1).unwrap();
    assert_eq!(
        world
            .get::<ecs::ActiveClients>(seat)
            .map(|clients| clients.0.clone()),
        Some(vec![c2])
    );
}

#[test]
fn acknowledging_one_disabled_client_preserves_other_active_clients() {
    let mut world = test_world();
    let seat = make_seat(&mut world);
    let c1 = make_client_with_pid(&mut world, 100);
    let c2 = make_client_with_pid(&mut world, 200);
    client::add(&mut world, seat, c1).unwrap();
    client::add(&mut world, seat, c2).unwrap();
    client::disable(&mut world, seat, c1).unwrap();
    client::ack_disable(&mut world, seat, c1).unwrap();
    assert_eq!(
        world
            .get::<ecs::ActiveClients>(seat)
            .map(|clients| clients.0.clone()),
        Some(vec![c2])
    );
}

#[test]
fn add_rejects_already_member() {
    let mut world = test_world();
    let seat = make_seat(&mut world);
    let client = make_client(&mut world);
    client::add(&mut world, seat, client).unwrap();
    assert!(client::add(&mut world, seat, client).is_err());
}

#[test]
fn is_active_true_for_active() {
    let mut world = test_world();
    let seat = make_seat(&mut world);
    let client = make_client(&mut world);
    client::add(&mut world, seat, client).unwrap();
    assert!(client::is_active(&world, seat, client));
}

#[test]
fn is_active_false_for_unrelated_entity() {
    let mut world = test_world();
    let seat = make_seat(&mut world);
    let c1 = make_client(&mut world);
    let other = world.spawn_empty().id();
    client::add(&mut world, seat, c1).unwrap();
    assert!(!client::is_active(&world, seat, other));
}

#[test]
fn disable_sets_pending_disabled() {
    let mut world = test_world();
    let seat = make_seat(&mut world);
    let client = make_client(&mut world);
    client::add(&mut world, seat, client).unwrap();
    client::disable(&mut world, seat, client).unwrap();
    assert_eq!(
        world.get::<ecs::SeatState>(client).map(|s| s.0),
        Some(seat::SeatState::PendingDisabled)
    );
}

#[test]
fn disable_rejects_non_enabled() {
    let mut world = test_world();
    let seat = make_seat(&mut world);
    let client = make_client(&mut world);
    client::add(&mut world, seat, client).unwrap();
    client::disable(&mut world, seat, client).unwrap();
    assert!(client::disable(&mut world, seat, client).is_err());
}

#[test]
fn ack_disable_sets_disabled() {
    let mut world = test_world();
    let seat = make_seat(&mut world);
    let c1 = make_client(&mut world);
    let c2 = make_client(&mut world);
    client::add(&mut world, seat, c1).unwrap();
    client::add(&mut world, seat, c2).unwrap();
    client::disable(&mut world, seat, c2).unwrap();
    client::ack_disable(&mut world, seat, c2).unwrap();
    assert_eq!(
        world.get::<ecs::SeatState>(c2).map(|s| s.0),
        Some(seat::SeatState::Disabled)
    );
}

#[test]
fn ack_disable_rejects_non_pending() {
    let mut world = test_world();
    let seat = make_seat(&mut world);
    let client = make_client(&mut world);
    client::add(&mut world, seat, client).unwrap();
    assert!(client::ack_disable(&mut world, seat, client).is_err());
}

#[test]
fn ack_disable_activates_next_queued_client() {
    let mut world = test_world();
    let seat = make_seat(&mut world);
    let c1 = make_client(&mut world);
    client::add(&mut world, seat, c1).unwrap();

    let c2 = make_client(&mut world);
    world
        .entity_mut(c2)
        .insert(ecs::SessionId(session::SessionId::from_raw(2)));
    world
        .entity_mut(c2)
        .insert(ecs::SeatName("seat0".to_string()));
    world.entity_mut(c2).insert(ecs::ClientSeat(seat));
    world.entity_mut(seat).insert(ecs::NextClients(vec![c2]));

    client::disable(&mut world, seat, c1).unwrap();
    client::ack_disable(&mut world, seat, c1).unwrap();

    assert_eq!(
        world.get::<ecs::ActiveClients>(seat).map(|a| a.0.clone()),
        Some(vec![c2])
    );
}

#[test]
fn set_next_session_switches_to_self_is_noop() {
    let mut world = test_world();
    let seat = make_seat(&mut world);
    let client = make_client(&mut world);
    client::add(&mut world, seat, client).unwrap();
    let session_id = world.get::<ecs::SessionId>(client).unwrap().0.as_raw();
    seat::set_next_session(&mut world, seat, client, session_id).unwrap();
    assert!(world.get::<ecs::NextClients>(seat).is_none());
}

#[test]
fn set_next_session_rejects_zero() {
    let mut world = test_world();
    let seat = make_seat(&mut world);
    let client = make_client(&mut world);
    client::add(&mut world, seat, client).unwrap();
    assert!(seat::set_next_session(&mut world, seat, client, 0).is_err());
}

#[test]
fn set_next_session_queues_switch() {
    let mut world = test_world();
    let seat = make_seat(&mut world);
    let c1 = make_client(&mut world);
    let c2 = make_client(&mut world);
    client::add(&mut world, seat, c1).unwrap();
    client::add(&mut world, seat, c2).unwrap();
    let c1_session = world.get::<ecs::SessionId>(c1).unwrap().0.as_raw();
    seat::set_next_session(&mut world, seat, c2, c1_session).unwrap();
    assert_eq!(
        world.get::<ecs::NextClients>(seat).map(|n| n.0.clone()),
        Some(vec![c1])
    );
}

#[test]
fn remove_client_cleans_up() {
    let mut world = test_world();
    let seat = make_seat(&mut world);
    let client = make_client(&mut world);
    client::add(&mut world, seat, client).unwrap();
    seat::remove_client(&mut world, seat, client).unwrap();
    assert!(world.get::<ecs::ActiveClients>(seat).is_none());
    assert!(seat::clients(&world, seat).is_empty());
}
