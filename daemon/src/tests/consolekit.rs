use crate::consolekit::{
    SessionInfo, append_database, is_active, key_file_bool, key_file_escape, key_file_list,
    seat_name, timestamp_string,
};
use crate::session::SessionId;
use crate::session_core_v1::ActiveState;
use std::time;

fn make_session(id: u32, uid: u32, seat: &str, active: ActiveState) -> SessionInfo {
    SessionInfo {
        id: SessionId::from_raw(id),
        uid,
        seat: seat.to_string(),
        timestamp: time::UNIX_EPOCH,
        is_local: true,
        active,
    }
}

#[test]
fn key_file_bool_true() {
    assert_eq!(key_file_bool(true), "true");
}

#[test]
fn key_file_bool_false() {
    assert_eq!(key_file_bool(false), "false");
}

#[test]
fn key_file_escape_plain_text() {
    assert_eq!(key_file_escape("hello"), "hello");
}

#[test]
fn key_file_escape_empty() {
    assert_eq!(key_file_escape(""), "");
}

#[test]
fn key_file_escape_special_chars() {
    assert_eq!(key_file_escape("\\"), "\\\\");
    assert_eq!(key_file_escape("\n"), "\\n");
    assert_eq!(key_file_escape("\r"), "\\r");
    assert_eq!(key_file_escape("\t"), "\\t");
    assert_eq!(key_file_escape(";"), "\\;");
}

#[test]
fn key_file_escape_mixed() {
    assert_eq!(key_file_escape("a;b\nc"), "a\\;b\\nc");
}

#[test]
fn key_file_list_empty() {
    let items: Vec<String> = vec![];
    assert_eq!(key_file_list(items.iter()), "");
}

#[test]
fn key_file_list_single() {
    let items = ["seat0".to_string()];
    assert_eq!(key_file_list(items.iter()), "seat0");
}

#[test]
fn key_file_list_multiple() {
    let items = ["seat0".to_string(), "seat1".to_string()];
    assert_eq!(key_file_list(items.iter()), "seat0;seat1");
}

#[test]
fn key_file_list_escapes_items() {
    let items = ["a;b".to_string()];
    assert_eq!(key_file_list(items.iter()), "a\\;b");
}

#[test]
fn timestamp_string_epoch() {
    assert_eq!(timestamp_string(time::UNIX_EPOCH), "0");
}

#[test]
fn timestamp_string_known_value() {
    let ts = time::UNIX_EPOCH + time::Duration::from_secs(1_000_000);
    assert_eq!(timestamp_string(ts), "1000000");
}

#[test]
fn seat_name_empty_defaults_to_seat0() {
    let session = make_session(1, 1000, "", ActiveState::Active);
    assert_eq!(seat_name(&session), "seat0");
}

#[test]
fn seat_name_non_empty_passthrough() {
    let session = make_session(1, 1000, "seat1", ActiveState::Active);
    assert_eq!(seat_name(&session), "seat1");
}

#[test]
fn is_active_active_state() {
    let session = make_session(1, 1000, "seat0", ActiveState::Active);
    assert!(is_active(&session));
}

#[test]
fn is_active_online_state() {
    let session = make_session(1, 1000, "seat0", ActiveState::Online);
    assert!(is_active(&session));
}

#[test]
fn is_active_closing_state() {
    let session = make_session(1, 1000, "seat0", ActiveState::Closing);
    assert!(!is_active(&session));
}

#[test]
fn is_active_opening_state() {
    let session = make_session(1, 1000, "seat0", ActiveState::Opening);
    assert!(!is_active(&session));
}

#[test]
fn append_database_empty() {
    let mut out = String::new();
    append_database(&mut out, &[]);
    assert!(out.contains("[Seats]"));
}

#[test]
fn append_database_single_session() {
    let sessions = vec![make_session(1, 1000, "seat0", ActiveState::Active)];
    let mut out = String::new();
    append_database(&mut out, &sessions);
    assert!(out.contains("[Session /org/freedesktop/ConsoleKit/Session/1]"));
    assert!(out.contains("uid=1000"));
    assert!(out.contains("seat=seat0"));
    assert!(out.contains("is_active=true"));
    assert!(out.contains("[Seat seat0]"));
    assert!(out.contains("[User 1000]"));
}

#[test]
fn append_database_deduplicates_seats() {
    let sessions = vec![
        make_session(1, 1000, "seat0", ActiveState::Active),
        make_session(2, 1001, "seat0", ActiveState::Online),
    ];
    let mut out = String::new();
    append_database(&mut out, &sessions);
    assert_eq!(out.matches("[Seat seat0]").count(), 1);
}
