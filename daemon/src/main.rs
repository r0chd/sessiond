mod moxsession_core_v1 {
    hyprwire::include_protocol!("moxsession_core_v1");
    pub use server::*;
}

use hyprwire::server;
use moxsession_core_v1::{moxsession_core_info, moxsession_core_manager};
use std::{env, fs, path};

struct Session {
    id: u32,
    uid: u32,
    username: String,
    tty: String,
}

struct Info {
    id: u32,
    object: moxsession_core_info::MoxsessionCoreInfo,
}

struct Moxsession {
    next_session_id: u32,
    sessions: Vec<Session>,
    infos: Vec<Info>,
}

impl moxsession_core_v1::MoxsessionCoreV1Handler for Moxsession {
    fn bind(&mut self, object: moxsession_core_manager::MoxsessionCoreManager) {
        let session_ids = self
            .sessions
            .iter()
            .map(|session| session.id)
            .collect::<Vec<_>>();
        object.send_sessions(&session_ids);
    }
}

impl hyprwire::Dispatch<moxsession_core_manager::MoxsessionCoreManager> for Moxsession {
    fn event(
        &mut self,
        object: &moxsession_core_manager::MoxsessionCoreManager,
        event: <moxsession_core_manager::MoxsessionCoreManager as hyprwire::Object>::Event<'_>,
    ) {
        let Some(client) = object.client() else {
            return;
        };

        match event {
            moxsession_core_manager::Event::CreateSession { uid, username, tty } => {
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
                };
                self.sessions.push(session);
                object.send_session_created(self.next_session_id);
                self.next_session_id = self.next_session_id.wrapping_add(1);
            }
            moxsession_core_manager::Event::DestroySession { session_id } => {
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

                    self.infos
                        .extract_if(.., |info| info.id == session.id)
                        .for_each(|info| info.object.send_destroyed());

                    return;
                }

                log::warn!(session_id; "destroy requested for unknown session");
            }
            moxsession_core_manager::Event::GetSessionInfo {
                moxsession_core_info,
                session_id,
            } => {
                if let Some(session) = self
                    .sessions
                    .iter()
                    .find(|session| session.id == session_id)
                {
                    log::debug!(session_id, uid = client.creds().uid.as_raw(); "session info requested");
                    moxsession_core_info.send_uid(session.uid);
                    moxsession_core_info.send_username(&session.username);
                    moxsession_core_info.send_tty(&session.tty);
                    moxsession_core_info.send_done();

                    let info = Info {
                        id: session_id,
                        object: moxsession_core_info,
                    };
                    self.infos.push(info);
                    return;
                }

                log::warn!(session_id; "get_session_info requested for unknown session");
                moxsession_core_info.send_destroyed();
            }
            moxsession_core_manager::Event::Destroy => {}
        }
    }
}

impl hyprwire::Dispatch<moxsession_core_info::MoxsessionCoreInfo> for Moxsession {
    fn event(
        &mut self,
        object: &moxsession_core_info::MoxsessionCoreInfo,
        event: <moxsession_core_info::MoxsessionCoreInfo as hyprwire::Object>::Event<'_>,
    ) {
        let moxsession_core_info::Event::Destroy = event;
        self.infos.retain(|info| info.object != *object);
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
        infos: Vec::new(),
    };

    let mut socket = server::Server::bind(&path)?;

    socket.add_implementation::<moxsession_core_v1::MoxsessionCoreV1Impl, _>(&mut moxsession, 1);

    loop {
        socket.dispatch_events(&mut moxsession, true)?;
    }
}
