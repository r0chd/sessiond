use crate::session_core_v1_client::{session_core_manager, session_core_session};
use crate::session_management_v1_client::session_manager;
use hyprwire::client;
use std::sync::atomic::{AtomicU32, Ordering};
use std::{path, process};

static SOCKET_ID: AtomicU32 = AtomicU32::new(0);

fn unique_socket() -> path::PathBuf {
    let id = SOCKET_ID.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("sessiond-test-{}-{id}.sock", process::id()))
}

fn daemon_binary() -> path::PathBuf {
    let exe = std::env::current_exe().unwrap();
    let mut dir = exe.parent().unwrap().to_path_buf();
    if dir.file_name().is_some_and(|n| n == "deps") {
        dir = dir.parent().unwrap().to_path_buf();
    }
    let candidate = dir.join("daemon");
    if candidate.exists() {
        return candidate;
    }
    // Under cargo-llvm-cov, tests run from target/llvm-cov-target/debug/deps/.
    // The regular daemon binary lives in the sibling target/debug/ dir.
    if let Some(fallback) = dir
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.join("debug/daemon"))
    {
        if fallback.exists() {
            return fallback;
        }
    }
    candidate
}

struct DaemonGuard(Option<(process::Child, path::PathBuf)>);

impl DaemonGuard {
    fn stop(mut self) {
        if let Some((mut child, socket_path)) = self.0.take() {
            let pid = rustix::process::Pid::from_raw(child.id() as i32).unwrap();
            let _ = rustix::process::kill_process(pid, rustix::process::Signal::TERM);
            let _ = child.wait();
            let _ = std::fs::remove_file(&socket_path);
        }
    }
}

impl Drop for DaemonGuard {
    fn drop(&mut self) {
        if let Some((mut child, socket_path)) = self.0.take() {
            let _ = child.kill();
            let _ = child.wait();
            let _ = std::fs::remove_file(&socket_path);
        }
    }
}

fn start_daemon(socket_path: path::PathBuf) -> DaemonGuard {
    let uid = rustix::process::getuid().as_raw().to_string();
    let gid = rustix::process::getgid().as_raw().to_string();
    let child = process::Command::new(daemon_binary())
        .arg("--socket-path")
        .arg(&socket_path)
        .arg("--management-user")
        .arg(&uid)
        .arg("--seat-user")
        .arg(&uid)
        .arg("--seat-group")
        .arg(&gid)
        .env("LOG_LEVEL", "off")
        .spawn()
        .expect("failed to spawn daemon");

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !socket_path.exists() {
        assert!(std::time::Instant::now() < deadline, "daemon did not start");
        std::thread::sleep(std::time::Duration::from_millis(1));
    }

    DaemonGuard(Some((child, socket_path)))
}

struct MgmtState {
    session_id: Option<u32>,
}

impl hyprwire::Dispatch<session_manager::SessionManager> for MgmtState {
    fn event(
        &mut self,
        _: &session_manager::SessionManager,
        event: <session_manager::SessionManager as hyprwire::Object>::Event<'_>,
    ) {
        let session_manager::Event::SessionCreated { session_id, .. } = event;
        self.session_id = Some(session_id);
    }
}

fn create_session(socket: &path::Path, uid: u32) -> u32 {
    let mut state = MgmtState { session_id: None };
    let mut sock = client::Client::connect(socket).unwrap();
    let eq = sock.new_event_queue();
    sock.add_implementation::<crate::session_management_v1_client::SessionManagementV1Impl>();
    eq.wait_for_handshake(&mut state).unwrap();
    let manager = sock
        .bind::<session_manager::SessionManager, _>(&eq, &mut state, 1)
        .unwrap();
    manager.send_create_session(uid, 0, 1, "");
    while state.session_id.is_none() {
        eq.dispatch_events(&mut state, true).unwrap();
    }
    state.session_id.unwrap()
}

fn destroy_session(socket: &path::Path, session_id: u32) {
    let mut state = MgmtState { session_id: None };
    let mut sock = client::Client::connect(socket).unwrap();
    let eq = sock.new_event_queue();
    sock.add_implementation::<crate::session_management_v1_client::SessionManagementV1Impl>();
    eq.wait_for_handshake(&mut state).unwrap();
    let manager = sock
        .bind::<session_manager::SessionManager, _>(&eq, &mut state, 1)
        .unwrap();
    manager.send_destroy_session(session_id);
}

struct CoreState {
    sessions: Vec<u32>,
    done: bool,
}

impl hyprwire::Dispatch<session_core_manager::SessionCoreManager> for CoreState {
    fn event(
        &mut self,
        _: &session_core_manager::SessionCoreManager,
        event: <session_core_manager::SessionCoreManager as hyprwire::Object>::Event<'_>,
    ) {
        match event {
            session_core_manager::Event::Sessions { session_ids } => {
                self.sessions.extend(session_ids);
            }
            session_core_manager::Event::Done => {
                self.done = true;
            }
            _ => {}
        }
    }
}

impl hyprwire::Dispatch<session_core_session::SessionCoreSession> for CoreState {
    fn event(
        &mut self,
        _: &session_core_session::SessionCoreSession,
        _: <session_core_session::SessionCoreSession as hyprwire::Object>::Event<'_>,
    ) {
    }
}

fn list_sessions(socket: &path::Path) -> Vec<u32> {
    let mut state = CoreState {
        sessions: Vec::new(),
        done: false,
    };
    let mut sock = client::Client::connect(socket).unwrap();
    let eq = sock.new_event_queue();
    sock.add_implementation::<crate::session_core_v1_client::SessionCoreV1Impl>();
    eq.wait_for_handshake(&mut state).unwrap();
    let _manager = sock
        .bind::<session_core_manager::SessionCoreManager, _>(&eq, &mut state, 1)
        .unwrap();
    while !state.done {
        eq.dispatch_events(&mut state, true).unwrap();
    }
    state.sessions
}

#[test]
fn create_session_returns_id() {
    let socket = unique_socket();
    let daemon = start_daemon(socket.clone());

    let id = create_session(&socket, 1000);
    assert_eq!(id, 1);

    daemon.stop();
}

#[test]
fn session_appears_in_core_list() {
    let socket = unique_socket();
    let daemon = start_daemon(socket.clone());

    create_session(&socket, 1000);
    let sessions = list_sessions(&socket);
    assert!(sessions.contains(&1));

    daemon.stop();
}

#[test]
fn sequential_sessions_get_incrementing_ids() {
    let socket = unique_socket();
    let daemon = start_daemon(socket.clone());

    let id1 = create_session(&socket, 1000);
    let id2 = create_session(&socket, 1001);
    assert_eq!(id1, 1);
    assert_eq!(id2, 2);

    daemon.stop();
}

// --- session-info helpers ---

struct SessionInfoState {
    uid: Option<u32>,
    username: Option<String>,
    session_id_field: Option<u32>,
    session_done: bool,
    session_destroyed: bool,
    locked: bool,
    unlocked: bool,
    ok: bool,
}

impl SessionInfoState {
    fn new() -> Self {
        Self {
            uid: None,
            username: None,
            session_id_field: None,
            session_done: false,
            session_destroyed: false,
            locked: false,
            unlocked: false,
            ok: false,
        }
    }
}

impl hyprwire::Dispatch<session_core_manager::SessionCoreManager> for SessionInfoState {
    fn event(
        &mut self,
        _: &session_core_manager::SessionCoreManager,
        _: <session_core_manager::SessionCoreManager as hyprwire::Object>::Event<'_>,
    ) {
    }
}

impl hyprwire::Dispatch<session_core_session::SessionCoreSession> for SessionInfoState {
    fn event(
        &mut self,
        _: &session_core_session::SessionCoreSession,
        event: <session_core_session::SessionCoreSession as hyprwire::Object>::Event<'_>,
    ) {
        match event {
            session_core_session::Event::Uid { uid } => self.uid = Some(uid),
            session_core_session::Event::Username { username } => {
                self.username = Some(username);
            }
            session_core_session::Event::SessionId { session_id } => {
                self.session_id_field = Some(session_id);
            }
            session_core_session::Event::Done => self.session_done = true,
            session_core_session::Event::Destroyed => self.session_destroyed = true,
            session_core_session::Event::Locked => self.locked = true,
            session_core_session::Event::Unlocked => self.unlocked = true,
            session_core_session::Event::Ok => self.ok = true,
            _ => {}
        }
    }
}

fn connect_core(socket: &path::Path) -> (client::Client, hyprwire::EventQueue, SessionInfoState) {
    let mut state = SessionInfoState::new();
    let mut sock = client::Client::connect(socket).unwrap();
    let eq = sock.new_event_queue();
    sock.add_implementation::<crate::session_core_v1_client::SessionCoreV1Impl>();
    eq.wait_for_handshake(&mut state).unwrap();
    (sock, eq, state)
}

#[test]
fn get_session_returns_fields() {
    let socket = unique_socket();
    let daemon = start_daemon(socket.clone());
    let uid = rustix::process::getuid().as_raw();
    let id = create_session(&socket, uid);

    let state = {
        let (sock, eq, mut state) = connect_core(&socket);
        let _manager = sock
            .bind::<session_core_manager::SessionCoreManager, _>(&eq, &mut state, 1)
            .unwrap();
        let _session = _manager.send_get_session::<SessionInfoState>(id).unwrap();
        while !state.session_done && !state.session_destroyed {
            eq.dispatch_events(&mut state, false).unwrap();
            if !state.session_done && !state.session_destroyed {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
        }
        state
    };

    assert!(state.session_done);
    assert_eq!(state.uid, Some(uid));
    assert!(state.username.is_some());
    assert_eq!(state.session_id_field, Some(id));

    daemon.stop();
}

#[test]
fn get_session_unknown_returns_destroyed() {
    let socket = unique_socket();
    let daemon = start_daemon(socket.clone());

    let state = {
        let (sock, eq, mut state) = connect_core(&socket);
        let _manager = sock
            .bind::<session_core_manager::SessionCoreManager, _>(&eq, &mut state, 1)
            .unwrap();
        let _session = _manager.send_get_session::<SessionInfoState>(999).unwrap();
        while !state.session_done && !state.session_destroyed {
            eq.dispatch_events(&mut state, false).unwrap();
            if !state.session_done && !state.session_destroyed {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
        }
        state
    };

    assert!(state.session_destroyed);

    daemon.stop();
}

#[test]
fn get_session_by_uid_returns_session() {
    let socket = unique_socket();
    let daemon = start_daemon(socket.clone());
    create_session(&socket, 1000);

    let state = {
        let (sock, eq, mut state) = connect_core(&socket);
        let _manager = sock
            .bind::<session_core_manager::SessionCoreManager, _>(&eq, &mut state, 1)
            .unwrap();
        let _session = _manager
            .send_get_session_by_uid::<SessionInfoState>(1000)
            .unwrap();
        while !state.session_done && !state.session_destroyed {
            eq.dispatch_events(&mut state, false).unwrap();
            if !state.session_done && !state.session_destroyed {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
        }
        state
    };

    assert!(state.session_done);
    assert_eq!(state.uid, Some(1000));

    daemon.stop();
}

#[test]
fn lock_unlock_session() {
    let socket = unique_socket();
    let daemon = start_daemon(socket.clone());
    let id = create_session(&socket, 1000);

    let (sock, eq, mut state) = connect_core(&socket);
    let _manager = sock
        .bind::<session_core_manager::SessionCoreManager, _>(&eq, &mut state, 1)
        .unwrap();
    let session = _manager.send_get_session::<SessionInfoState>(id).unwrap();
    while !state.session_done && !state.session_destroyed {
        eq.dispatch_events(&mut state, false).unwrap();
        if !state.session_done && !state.session_destroyed {
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }
    assert!(state.session_done);

    session.send_lock();
    while !state.locked {
        eq.dispatch_events(&mut state, false).unwrap();
        if !state.locked {
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }
    assert!(state.ok);

    state.unlocked = false;
    state.ok = false;
    session.send_unlock();
    while !state.unlocked {
        eq.dispatch_events(&mut state, false).unwrap();
        if !state.unlocked {
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }
    assert!(state.ok);

    drop((sock, eq, session, _manager));
    daemon.stop();
}

// --- destroy test ---

#[test]
fn destroy_session_removes_from_list() {
    let socket = unique_socket();
    let daemon = start_daemon(socket.clone());

    let id = create_session(&socket, 1000);
    destroy_session(&socket, id);

    // Poll until the daemon has processed the destroy — fire-and-forget means
    // the message may not be processed before our next list_sessions connect.
    let mut attempts = 0u32;
    loop {
        let sessions = list_sessions(&socket);
        if !sessions.contains(&id) {
            break;
        }
        attempts += 1;
        assert!(
            attempts < 50,
            "session {id} not removed after {attempts} polls"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    daemon.stop();
}
