mod session_management_v1 {
    hyprwire::include_protocol!("session_management_v1");
    pub use server::*;
}

mod session_core_v1 {
    hyprwire::include_protocol!("session_core_v1");
    pub use server::*;
    pub use spec::*;
}

mod cgroup;

use calloop::generic;
use hyprwire::server;
use session_core_v1::{session_core_manager, session_core_session};
use session_management_v1::session_manager;
use std::os::fd::AsRawFd;
use std::os::unix::fs::PermissionsExt;
use std::{env, fs, path, time};

struct Session {
    id: u32,
    uid: u32,
    username: String,
    tty: String,
    seat: Option<String>,
    timestamp: time::SystemTime,
    leader_id: i32,
    objects: Vec<session_core_session::SessionCoreSession>,
    state: session_core_v1::ActiveState,
}

impl Session {
    fn set_state(&mut self, state: session_core_v1::ActiveState) {
        log::info!(session_id = self.id, tty = self.tty.as_str(); "session state changed");
        self.state = state;
        for object in self.objects.iter() {
            object.send_active(self.state as u32);
        }
    }

    fn add_object(&mut self, object: session_core_session::SessionCoreSession) {
        log::debug!(session_id = self.id, uid = object.client().map(|c| c.creds().uid.as_raw()).unwrap_or(0); "session info requested");
        object.send_session_id(self.id);
        object.send_uid(self.uid);
        object.send_username(&self.username);
        object.send_tty(&self.tty);
        object.send_active(self.state as u32);
        if let Some(seat) = &self.seat {
            object.send_seat(seat);
        }
        object.send_leader(self.leader_id as u32);
        object.send_timestamp(
            self.timestamp
                .duration_since(time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as u32,
        );
        object.send_done();

        self.objects.push(object);
    }
}

struct Sessiond {
    active_tty: Option<String>,
    next_session_id: u32,
    sessions: Vec<Session>,
    info_managers: Vec<session_core_manager::SessionCoreManager>,
    cgroup_manager: cgroup::CgroupManager,
}

impl session_management_v1::SessionManagementV1Handler for Sessiond {}

impl session_core_v1::SessionCoreV1Handler for Sessiond {
    fn bind(&mut self, object: session_core_manager::SessionCoreManager) {
        let session_ids = self
            .sessions
            .iter()
            .map(|session| session.id)
            .collect::<Vec<_>>();
        object.send_sessions(&session_ids);
        self.info_managers.push(object);
    }
}

impl hyprwire::Dispatch<session_manager::SessionManager> for Sessiond {
    fn event(
        &mut self,
        object: &session_manager::SessionManager,
        event: <session_manager::SessionManager as hyprwire::Object>::Event<'_>,
    ) {
        let Some(client) = object.client() else {
            return;
        };

        match event {
            session_manager::Event::CreateSession { uid, username, tty } => {
                if !client.creds().uid.is_root() {
                    log::warn!(uid = client.creds().uid.as_raw(); "rejected request from non-root client");
                    return;
                }

                log::info!(session_id = self.next_session_id, uid, username = username.as_str(), tty = tty.as_str(); "session created");

                let leader_id = client.creds().pid.as_raw_pid();

                if let Err(e) = self
                    .cgroup_manager
                    .create_session_cgroup(self.next_session_id, leader_id)
                {
                    log::error!(session_id = self.next_session_id, error = e.to_string().as_str(); "failed to create session cgroup");
                }

                let mut session = Session {
                    id: self.next_session_id,
                    uid,
                    username,
                    tty,
                    seat: None,
                    timestamp: time::SystemTime::now(),
                    leader_id,
                    objects: Vec::new(),
                    state: session_core_v1::ActiveState::Online,
                };
                if self
                    .active_tty
                    .as_ref()
                    .is_some_and(|active_tty| *active_tty == session.tty)
                {
                    session.set_state(session_core_v1::ActiveState::Active);
                }

                object.send_session_created(self.next_session_id);
                self.sessions.push(session);

                for manager in &self.info_managers {
                    manager.send_session_added(self.next_session_id);
                }

                self.next_session_id = self.next_session_id.wrapping_add(1);
            }
            session_manager::Event::DestroySession { session_id } => {
                if !client.creds().uid.is_root() {
                    log::warn!(uid = client.creds().uid.as_raw(); "rejected request from non-root client");
                    return;
                }

                if let Some(mut session) = self
                    .sessions
                    .extract_if(.., |session| session.id == session_id)
                    .next()
                {
                    log::info!(session_id, uid = session.uid, username = session.username.as_str(); "session destroyed");

                    session.set_state(session_core_v1::ActiveState::Closing);
                    for object in session.objects.iter() {
                        object.send_destroyed();
                    }

                    for manager in &self.info_managers {
                        manager.send_session_removed(session_id);
                    }

                    if let Err(e) = self.cgroup_manager.poll_destroy(session_id) {
                        log::error!(session_id, error = e.to_string().as_str(); "failed to poll destroy session cgroup");
                    }

                    return;
                }

                log::warn!(session_id; "destroy requested for unknown session");
            }
            session_manager::Event::Destroy => {}
        }
    }
}

impl hyprwire::Dispatch<session_core_manager::SessionCoreManager> for Sessiond {
    fn event(
        &mut self,
        object: &session_core_manager::SessionCoreManager,
        event: <session_core_manager::SessionCoreManager as hyprwire::Object>::Event<'_>,
    ) {
        match event {
            session_core_manager::Event::GetSession {
                session_core_session,
                session_id,
            } => {
                if let Some(session) = self
                    .sessions
                    .iter_mut()
                    .find(|session| session.id == session_id)
                {
                    session.add_object(session_core_session);
                    return;
                }

                log::warn!(session_id; "get_session requested for unknown session");
                session_core_session.send_destroyed();
            }
            session_core_manager::Event::GetSessionByPid {
                session_core_session,
                pid,
            } => {
                let Ok(Some(session_id)) = self.cgroup_manager.find_session_by_pid(pid) else {
                    return;
                };

                if let Some(session) = self
                    .sessions
                    .iter_mut()
                    .find(|session| session.id == session_id)
                {
                    session.add_object(session_core_session);
                    return;
                }

                log::warn!(session_id; "get_session requested for unknown session");
                session_core_session.send_destroyed();
            }
            session_core_manager::Event::Destroy => {
                self.info_managers.retain(|m| m != object);
            }
        }
    }
}

impl hyprwire::Dispatch<session_core_session::SessionCoreSession> for Sessiond {
    fn event(
        &mut self,
        object: &session_core_session::SessionCoreSession,
        event: <session_core_session::SessionCoreSession as hyprwire::Object>::Event<'_>,
    ) {
        let Some(idx) = self.sessions.iter_mut().position(|session| {
            session
                .objects
                .iter()
                .any(|session_object| session_object == object)
        }) else {
            return;
        };

        if let session_core_session::Event::Destroy = event {
            self.sessions[idx]
                .objects
                .retain(|session_object| session_object != object);
            return;
        }

        let Some(client) = object.client() else {
            return;
        };
        if !client.creds().uid.is_root() && client.creds().uid.as_raw() != self.sessions[idx].uid {
            object.send_error(
                session_core_v1::ErrorCode::PermissionDenied as u32,
                "permission denied",
            );
            return;
        }

        match event {
            session_core_session::Event::Terminate => {
                let session = self.sessions.swap_remove(idx);

                log::info!(session_id = session.id; "terminate requested");
                for obj in session.objects.iter() {
                    obj.send_active(session_core_v1::ActiveState::Closing as u32);
                }
                if let Err(err) = self.cgroup_manager.terminate_session(session.id) {
                    log::error!(session_id = session.id, err = err.to_string().as_str(); "terminate failed");
                }
            }
            session_core_session::Event::Kill => {
                log::info!(session_id = self.sessions[idx].id; "kill requested");
                for obj in self.sessions[idx].objects.iter() {
                    obj.send_active(session_core_v1::ActiveState::Closing as u32);
                }
                if let Err(err) = self.cgroup_manager.kill_session(self.sessions[idx].id) {
                    log::error!(session_id = self.sessions[idx].id, err = err.to_string().as_str(); "kill failed");
                }
                self.sessions.swap_remove(idx);
            }
            session_core_session::Event::Lock => {
                log::info!(session_id = self.sessions[idx].id; "lock requested");
                for object in self.sessions[idx].objects.iter() {
                    object.send_locked();
                }
            }
            session_core_session::Event::Unlock => {
                log::info!(session_id = self.sessions[idx].id; "unlock requested");
                for object in self.sessions[idx].objects.iter() {
                    object.send_unlocked();
                }
            }
            _ => unreachable!(),
        }
    }
}

fn main() -> anyhow::Result<()> {
    let log_level = match env::var("LOG_LEVEL") {
        Ok(log) if log == "trace" => log::LevelFilter::Trace,
        Ok(log) if log == "debug" => log::LevelFilter::Debug,
        Ok(log) if log == "warn" => log::LevelFilter::Warn,
        Ok(log) if log == "error" => log::LevelFilter::Error,
        Ok(log) if log == "off" => log::LevelFilter::Off,
        _ => log::LevelFilter::Info,
    };

    env_logger::Builder::new()
        .filter(Some("daemon"), log_level)
        .init();

    log::info!("sessiond daemon starting");

    let mut event_loop = calloop::EventLoop::try_new()?;

    let cgroup_manager = cgroup::CgroupManager::new(event_loop.handle())?;

    let mut sessiond = Sessiond {
        active_tty: None,
        next_session_id: 1,
        sessions: Vec::new(),
        info_managers: Vec::new(),
        cgroup_manager,
    };

    let path = path::Path::new("/run/sessiond.sock");
    let mut socket = server::Server::bind(&path)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o777))?;

    socket
        .add_implementation::<session_management_v1::SessionManagementV1Impl, _>(&mut sessiond, 1);
    socket.add_implementation::<session_core_v1::SessionCoreV1Impl, _>(&mut sessiond, 1);

    let fd_wrapper = unsafe { generic::FdWrapper::new(socket.extract_loop_fd().as_raw_fd()) };
    let source = generic::Generic::new(
        fd_wrapper,
        calloop::Interest {
            readable: true,
            writable: false,
        },
        calloop::Mode::Level,
    );
    event_loop
        .handle()
        .insert_source(source, move |_, _, state| {
            _ = socket.dispatch_events(state, false);
            Ok(calloop::PostAction::Continue)
        })
        .unwrap();

    let tty0_path = path::Path::new("/sys/class/tty/tty0/active");
    let source = generic::Generic::new(
        fs::File::open(tty0_path)?,
        calloop::Interest {
            readable: true,
            writable: false,
        },
        calloop::Mode::Level,
    );
    event_loop
        .handle()
        .insert_source(source, move |_, _, state| {
            let active = match fs::read_to_string(tty0_path) {
                Ok(active) => active,
                Err(e) => {
                    log::error!(error = e.to_string().as_str(); "failed to read active tty");
                    return Ok(calloop::PostAction::Continue);
                }
            };
            let active = active.trim();

            if let Some(session) = state.sessions.iter_mut().find(|session| {
                state
                    .active_tty
                    .as_ref()
                    .is_some_and(|tty| *tty == session.tty)
            }) {
                if session.tty == active {
                    return Ok(calloop::PostAction::Continue);
                }

                session.set_state(session_core_v1::ActiveState::Online);
            }

            state.active_tty = Some(active.to_string());

            if let Some(session) = state
                .sessions
                .iter_mut()
                .find(|session| session.tty == active)
            {
                session.set_state(session_core_v1::ActiveState::Active);
            }

            Ok(calloop::PostAction::Continue)
        })
        .unwrap();

    event_loop.run(None, &mut sessiond, |_| {})?;

    Ok(())
}
