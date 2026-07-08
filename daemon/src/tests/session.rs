use crate::session::SessionId;

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
