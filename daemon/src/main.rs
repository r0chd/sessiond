mod moxsession_pam_v1 {
    hyprwire::include_protocol!("moxsession_pam_v1");
    pub use server::*;
}

mod moxsession_info_v1 {
    hyprwire::include_protocol!("moxsession_info_v1");
    pub use server::*;
}

use hyprwire::server;
use moxsession_info_v1::{moxsession_info_manager, moxsession_info_session};
use moxsession_pam_v1::moxsession_pam_manager;
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
    object: moxsession_info_session::MoxsessionInfoSession,
}

struct Moxsession {
    next_session_id: u32,
    sessions: Vec<Session>,
    info_sessions: Vec<InfoSession>,
    info_managers: Vec<moxsession_info_manager::MoxsessionInfoManager>,
}

impl moxsession_pam_v1::MoxsessionPamV1Handler for Moxsession {}

impl moxsession_info_v1::MoxsessionInfoV1Handler for Moxsession {
    fn bind(&mut self, object: moxsession_info_manager::MoxsessionInfoManager) {
        let session_ids = self
            .sessions
            .iter()
            .map(|session| session.id)
            .collect::<Vec<_>>();
        object.send_sessions(&session_ids);
        self.info_managers.push(object);
    }
}

impl hyprwire::Dispatch<moxsession_pam_manager::MoxsessionPamManager> for Moxsession {
    fn event(
        &mut self,
        object: &moxsession_pam_manager::MoxsessionPamManager,
        event: <moxsession_pam_manager::MoxsessionPamManager as hyprwire::Object>::Event<'_>,
    ) {
        let Some(client) = object.client() else {
            return;
        };

        match event {
            moxsession_pam_manager::Event::CreateSession { uid, username, tty } => {
                if !client.creds().uid.is_root() {
                    log::warn!(uid = client.creds().uid.as_raw(); "rejected request from non-root client");
                    return;
                }

                log::info!(session_id = self.next_session_id, uid, username = username.as_str(), tty = tty.as_str(); "session created");

                let session = Session {
                    id: self.next_session_id,
                    uid,
                    username,
                    tty,
                    seat: None,
                    timestamp: time::SystemTime::now(),
                    leader_id: client.creds().pid.as_raw_pid(),
                };
                self.sessions.push(session);
                object.send_session_created(self.next_session_id);

                for manager in &self.info_managers {
                    manager.send_session_added(self.next_session_id);
                }

                self.next_session_id = self.next_session_id.wrapping_add(1);
            }
            moxsession_pam_manager::Event::DestroySession { session_id } => {
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

                    return;
                }

                log::warn!(session_id; "destroy requested for unknown session");
            }
            moxsession_pam_manager::Event::Destroy => {}
        }
    }
}

impl hyprwire::Dispatch<moxsession_info_manager::MoxsessionInfoManager> for Moxsession {
    fn event(
        &mut self,
        object: &moxsession_info_manager::MoxsessionInfoManager,
        event: <moxsession_info_manager::MoxsessionInfoManager as hyprwire::Object>::Event<'_>,
    ) {
        match event {
            moxsession_info_manager::Event::GetSessionInfo {
                moxsession_info_session,
                session_id,
            } => {
                if let Some(session) = self
                    .sessions
                    .iter()
                    .find(|session| session.id == session_id)
                {
                    log::debug!(session_id, uid = object.client().map(|c| c.creds().uid.as_raw()).unwrap_or(0); "session info requested");
                    moxsession_info_session.send_uid(session.uid);
                    moxsession_info_session.send_username(&session.username);
                    moxsession_info_session.send_tty(&session.tty);
                    if let Some(seat) = &session.seat {
                        moxsession_info_session.send_seat(seat);
                    }
                    moxsession_info_session.send_leader(session.leader_id as u32);
                    moxsession_info_session.send_timestamp(
                        session
                            .timestamp
                            .duration_since(time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs() as u32,
                    );
                    moxsession_info_session.send_done();

                    self.info_sessions.push(InfoSession {
                        id: session_id,
                        object: moxsession_info_session,
                    });
                    return;
                }

                log::warn!(session_id; "get_session_info requested for unknown session");
                moxsession_info_session.send_destroyed();
            }
            moxsession_info_manager::Event::Destroy => {
                self.info_managers.retain(|m| m != object);
            }
        }
    }
}

impl hyprwire::Dispatch<moxsession_info_session::MoxsessionInfoSession> for Moxsession {
    fn event(
        &mut self,
        object: &moxsession_info_session::MoxsessionInfoSession,
        event: <moxsession_info_session::MoxsessionInfoSession as hyprwire::Object>::Event<'_>,
    ) {
        let moxsession_info_session::Event::Destroy = event;
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

    log::info!("moxsession daemon starting");

    let path = path::Path::new("/run/mox/moxsession.sock");
    fs::create_dir_all(path.parent().unwrap())?;

    let mut moxsession = Moxsession {
        next_session_id: 0,
        sessions: Vec::new(),
        info_sessions: Vec::new(),
        info_managers: Vec::new(),
    };

    let mut socket = server::Server::bind(&path)?;

    fs::set_permissions(path, fs::Permissions::from_mode(0o777))?;

    socket.add_implementation::<moxsession_pam_v1::MoxsessionPamV1Impl, _>(&mut moxsession, 1);
    socket.add_implementation::<moxsession_info_v1::MoxsessionInfoV1Impl, _>(&mut moxsession, 1);

    loop {
        socket.dispatch_events(&mut moxsession, true)?;
    }
}
