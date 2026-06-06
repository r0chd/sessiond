mod session_management_v1 {
    hyprwire::include_protocol!("session_management_v1");
    pub use server::*;
}

mod session_core_v1 {
    hyprwire::include_protocol!("session_core_v1");
    pub use server::*;
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
}

struct InfoSession {
    id: u32,
    object: session_core_session::SessionCoreSession,
}

struct Sessiond {
    next_session_id: u32,
    sessions: Vec<Session>,
    info_sessions: Vec<InfoSession>,
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

                let session = Session {
                    id: self.next_session_id,
                    uid,
                    username,
                    tty,
                    seat: None,
                    timestamp: time::SystemTime::now(),
                    leader_id,
                };
                self.sessions.push(session);
                object.send_session_created(self.next_session_id);

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

                if let Some(session) = self
                    .sessions
                    .extract_if(.., |session| session.id == session_id)
                    .next()
                {
                    log::info!(session_id, uid = session.uid, username = session.username.as_str(); "session destroyed");

                    self.info_sessions
                        .extract_if(.., |info| info.id == session.id)
                        .for_each(|info| info.object.send_destroyed());

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
                    .iter()
                    .find(|session| session.id == session_id)
                {
                    log::debug!(session_id, uid = object.client().map(|c| c.creds().uid.as_raw()).unwrap_or(0); "session info requested");
                    session_core_session.send_session_id(session_id);
                    session_core_session.send_uid(session.uid);
                    session_core_session.send_username(&session.username);
                    session_core_session.send_tty(&session.tty);
                    if let Some(seat) = &session.seat {
                        session_core_session.send_seat(seat);
                    }
                    session_core_session.send_leader(session.leader_id as u32);
                    session_core_session.send_timestamp(
                        session
                            .timestamp
                            .duration_since(time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs() as u32,
                    );
                    session_core_session.send_done();

                    self.info_sessions.push(InfoSession {
                        id: session_id,
                        object: session_core_session,
                    });
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
                    .iter()
                    .find(|session| session.id == session_id)
                {
                    log::debug!(session_id, uid = object.client().map(|c| c.creds().uid.as_raw()).unwrap_or(0); "session info requested");
                    session_core_session.send_session_id(session_id);
                    session_core_session.send_uid(session.uid);
                    session_core_session.send_username(&session.username);
                    session_core_session.send_tty(&session.tty);
                    if let Some(seat) = &session.seat {
                        session_core_session.send_seat(seat);
                    }
                    session_core_session.send_leader(session.leader_id as u32);
                    session_core_session.send_timestamp(
                        session
                            .timestamp
                            .duration_since(time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs() as u32,
                    );
                    session_core_session.send_done();

                    self.info_sessions.push(InfoSession {
                        id: session_id,
                        object: session_core_session,
                    });
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
        let session_core_session::Event::Destroy = event;
        self.info_sessions.retain(|info| info.object != *object);
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
        next_session_id: 1,
        sessions: Vec::new(),
        info_sessions: Vec::new(),
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

    event_loop.run(None, &mut sessiond, |_| {})?;

    Ok(())
}
