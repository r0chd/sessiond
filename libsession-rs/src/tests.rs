#![allow(clippy::unwrap_used)]
#![allow(clippy::panic)]
#![allow(clippy::expect_used)]
#![allow(clippy::cast_possible_wrap)]
use super::*;
use std::{env, process};

#[test]
fn connects_to_correct_backend() {
    let context = Context::connect().expect("Failed to connect to any backend");
    let backend = context
        .get_backend()
        .expect("Failed to get current backend");

    if let Ok(env_backend) = env::var("LIBSESSION_BACKEND") {
        match env_backend.as_str() {
            "sessiond" => {
                assert_eq!(backend, libsession_sys::LibsessionBackend::Sessiond);
            }
            "logind" => {
                assert_eq!(backend, libsession_sys::LibsessionBackend::Logind);
            }
            "consolekit" => {
                assert_eq!(backend, libsession_sys::LibsessionBackend::ConsoleKit);
            }
            _ => panic!("unsupported LIBSESSION_BACKEND value: {env_backend:?}"),
        }
    } else {
        let running = |name: &str| {
            process::Command::new("pidof")
                .arg(name)
                .output()
                .expect("failed to execute pidof")
                .status
                .success()
        };

        let expected = if cfg!(feature = "sessiond") && running("sessiond") {
            libsession_sys::LibsessionBackend::Sessiond
        } else if cfg!(feature = "logind") && running("systemd-logind") {
            libsession_sys::LibsessionBackend::Logind
        } else if cfg!(feature = "consolekit") && running("console-kit-daemon") {
            libsession_sys::LibsessionBackend::ConsoleKit
        } else {
            panic!("no supported backend daemon is running");
        };

        assert_eq!(
            backend, expected,
            "selected backend does not match the running daemon"
        );
    }
}

#[test]
fn list_sessions() {
    let context = Context::connect().expect("no session backend available");

    let current_id = context
        .get_session()
        .expect("current process has no session")
        .id()
        .expect("failed to read current session ID");

    let ids: Vec<_> = context
        .list_sessions()
        .expect("failed to list sessions")
        .iter()
        .map(|session| session.id().expect("failed to read session ID"))
        .collect();

    assert!(
        ids.contains(&current_id),
        "current session {current_id:?} missing from session list: {ids:?}"
    );
}

#[test]
fn is_local() {
    let context = Context::connect().expect("no session backend available");
    let session = context
        .get_session()
        .expect("current process has no session");

    let over_ssh = ["SSH_CONNECTION", "SSH_CLIENT", "SSH_TTY"]
        .into_iter()
        .any(|variable| env::var_os(variable).is_some());

    let is_local = context
        .is_session_local(&session)
        .expect("failed to check session locality");

    assert_ne!(is_local, over_ssh);
}

#[test]
fn get_owner_uid_for_pid() {
    let context = Context::connect().expect("no session backend available");
    let pid = process::id() as i32;

    let uid = context.get_owner_uid_for_pid(pid).unwrap();
    assert_eq!(uid, 1000);
}
