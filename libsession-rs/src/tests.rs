use super::*;
use std::process;

#[test]
fn connects() {
    assert!(Context::connect().is_some());
}

#[test]
fn list_sessions() {
    let context = Context::connect().expect("no session backend available");
    let sessions = context
        .list_sessions()
        .expect("current session lookup failed");

    for session in sessions {
        let id = session.id().unwrap();
        println!("{id}");
    }
}

#[test]
fn is_local() {
    let context = Context::connect().expect("no session backend available");
    let sessions = context
        .list_sessions()
        .expect("current session lookup failed");

    for session in sessions {
        let is_local = context.is_session_local(&session).unwrap();
        println!("{is_local}");
    }
}

#[test]
fn get_owner_uid_for_pid() {
    let context = Context::connect().expect("no session backend available");
    let pid = process::id() as i32;

    let uid = context.get_owner_uid_for_pid(pid).unwrap();
    assert_eq!(uid, 1000);
}
