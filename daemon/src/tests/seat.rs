use crate::ecs::{self, SeatKind, SeatState as EcsSeatState};
use crate::seat::{self, SeatState};
use crate::session::SessionId;

fn make_seat(world: &mut ecs::World) -> ecs::Entity {
    world.insert_seat("seat0".to_string(), SeatKind::NonVtBound, 0)
}

fn make_client(world: &mut ecs::World) -> ecs::Entity {
    let e = world.entity();
    world.insert(e, ecs::Devices(Vec::new()));
    world.insert(e, EcsSeatState(SeatState::New));
    e
}

#[test]
fn add_client_assigns_session_id_1() {
    let mut world = ecs::World::new();
    let seat = make_seat(&mut world);
    let client = make_client(&mut world);
    seat::add_client(&mut world, seat, client).unwrap();
    assert_eq!(world.get::<ecs::Session>(client).map(|s| s.0.as_raw()), Some(1));
}

#[test]
fn add_client_assigns_sequential_ids() {
    let mut world = ecs::World::new();
    let seat = make_seat(&mut world);
    let c1 = make_client(&mut world);
    let c2 = make_client(&mut world);
    seat::add_client(&mut world, seat, c1).unwrap();
    seat::add_client(&mut world, seat, c2).unwrap();
    assert_eq!(world.get::<ecs::Session>(c1).map(|s| s.0.as_raw()), Some(1));
    assert_eq!(world.get::<ecs::Session>(c2).map(|s| s.0.as_raw()), Some(2));
}

#[test]
fn add_client_sets_enabled_state() {
    let mut world = ecs::World::new();
    let seat = make_seat(&mut world);
    let client = make_client(&mut world);
    seat::add_client(&mut world, seat, client).unwrap();
    assert_eq!(
        world.get::<EcsSeatState>(client).map(|s| s.0),
        Some(SeatState::Enabled)
    );
}

#[test]
fn add_client_sets_active_client() {
    let mut world = ecs::World::new();
    let seat = make_seat(&mut world);
    let client = make_client(&mut world);
    seat::add_client(&mut world, seat, client).unwrap();
    assert_eq!(world.get::<ecs::ActiveClient>(seat).map(|a| a.0), Some(client));
}

#[test]
fn add_client_rejects_already_member() {
    let mut world = ecs::World::new();
    let seat = make_seat(&mut world);
    let client = make_client(&mut world);
    seat::add_client(&mut world, seat, client).unwrap();
    assert!(seat::add_client(&mut world, seat, client).is_err());
}

#[test]
fn is_active_client_true_for_active() {
    let mut world = ecs::World::new();
    let seat = make_seat(&mut world);
    let client = make_client(&mut world);
    seat::add_client(&mut world, seat, client).unwrap();
    assert!(seat::is_active_client(&world, seat, client));
}

#[test]
fn is_active_client_false_for_unrelated_entity() {
    let mut world = ecs::World::new();
    let seat = make_seat(&mut world);
    let c1 = make_client(&mut world);
    let other = world.entity();
    seat::add_client(&mut world, seat, c1).unwrap();
    assert!(!seat::is_active_client(&world, seat, other));
}

#[test]
fn disable_client_sets_pending_disabled() {
    let mut world = ecs::World::new();
    let seat = make_seat(&mut world);
    let client = make_client(&mut world);
    seat::add_client(&mut world, seat, client).unwrap();
    seat::disable_client(&mut world, seat, client).unwrap();
    assert_eq!(
        world.get::<EcsSeatState>(client).map(|s| s.0),
        Some(SeatState::PendingDisabled)
    );
}

#[test]
fn disable_client_rejects_non_enabled() {
    let mut world = ecs::World::new();
    let seat = make_seat(&mut world);
    let client = make_client(&mut world);
    seat::add_client(&mut world, seat, client).unwrap();
    seat::disable_client(&mut world, seat, client).unwrap();
    assert!(seat::disable_client(&mut world, seat, client).is_err());
}

#[test]
fn ack_disable_client_sets_disabled() {
    let mut world = ecs::World::new();
    let seat = make_seat(&mut world);
    let c1 = make_client(&mut world);
    let c2 = make_client(&mut world);
    // add both; c2 becomes active
    seat::add_client(&mut world, seat, c1).unwrap();
    seat::add_client(&mut world, seat, c2).unwrap();
    seat::disable_client(&mut world, seat, c2).unwrap();
    seat::ack_disable_client(&mut world, seat, c2).unwrap();
    // c2 should be Disabled; activate() tries c1 but c1 is Enabled, not New/Disabled, so no re-enable
    assert_eq!(
        world.get::<EcsSeatState>(c2).map(|s| s.0),
        Some(SeatState::Disabled)
    );
}

#[test]
fn ack_disable_rejects_non_pending() {
    let mut world = ecs::World::new();
    let seat = make_seat(&mut world);
    let client = make_client(&mut world);
    seat::add_client(&mut world, seat, client).unwrap();
    assert!(seat::ack_disable_client(&mut world, seat, client).is_err());
}

#[test]
fn ack_disable_activates_next_queued_client() {
    let mut world = ecs::World::new();
    let seat = make_seat(&mut world);
    let c1 = make_client(&mut world);
    seat::add_client(&mut world, seat, c1).unwrap(); // c1 active, session=1

    // manually configure c2 with the components open_client needs
    let c2 = make_client(&mut world);
    world.insert(c2, ecs::Session(SessionId::from_raw(2)));
    world.insert(c2, ecs::SeatName("seat0".to_string()));
    world.insert(c2, ecs::SeatEntity(seat));
    world.insert(seat, ecs::NextClient(c2));

    seat::disable_client(&mut world, seat, c1).unwrap();
    seat::ack_disable_client(&mut world, seat, c1).unwrap();

    assert_eq!(world.get::<ecs::ActiveClient>(seat).map(|a| a.0), Some(c2));
}

#[test]
fn set_next_session_switches_to_self_is_noop() {
    let mut world = ecs::World::new();
    let seat = make_seat(&mut world);
    let client = make_client(&mut world);
    seat::add_client(&mut world, seat, client).unwrap();
    let session_id = world.get::<ecs::Session>(client).unwrap().0.as_raw();
    seat::set_next_session(&mut world, seat, client, session_id).unwrap();
    assert!(world.get::<ecs::NextClient>(seat).is_none());
}

#[test]
fn set_next_session_rejects_zero() {
    let mut world = ecs::World::new();
    let seat = make_seat(&mut world);
    let client = make_client(&mut world);
    seat::add_client(&mut world, seat, client).unwrap();
    assert!(seat::set_next_session(&mut world, seat, client, 0).is_err());
}

#[test]
fn set_next_session_queues_switch() {
    let mut world = ecs::World::new();
    let seat = make_seat(&mut world);
    let c1 = make_client(&mut world);
    let c2 = make_client(&mut world);
    seat::add_client(&mut world, seat, c1).unwrap(); // c1: session=1
    seat::add_client(&mut world, seat, c2).unwrap(); // c2: session=2, now active
    let c1_session = world.get::<ecs::Session>(c1).unwrap().0.as_raw();
    // switch from c2 (active) to c1
    seat::set_next_session(&mut world, seat, c2, c1_session).unwrap();
    assert_eq!(world.get::<ecs::NextClient>(seat).map(|n| n.0), Some(c1));
}

#[test]
fn remove_client_cleans_up() {
    let mut world = ecs::World::new();
    let seat = make_seat(&mut world);
    let client = make_client(&mut world);
    seat::add_client(&mut world, seat, client).unwrap();
    seat::remove_client(&mut world, seat, client).unwrap();
    assert!(world.get::<ecs::ActiveClient>(seat).is_none());
    assert!(world.get::<ecs::SeatClients>(seat).is_some_and(|c| c.0.is_empty()));
}
