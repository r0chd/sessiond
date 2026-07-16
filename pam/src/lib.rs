mod session_management_v1 {
    hyprwire::include_protocol!("session_management_v1");
    pub use client::*;
}

use hyprwire::client;
use pamsm::{PamLibExt, PamServiceModule};
use session_management_v1::session_manager;

struct Session {
    id: Option<u32>,
    seat: Option<String>,
    env: Vec<(String, String)>,
    done: bool,
}

impl hyprwire::Dispatch<session_manager::SessionManager> for Session {
    fn event(
        &mut self,
        _: &session_manager::SessionManager,
        event: <session_manager::SessionManager as hyprwire::Object>::Event<'_>,
    ) {
        match event {
            session_manager::Event::SessionId { session_id } => {
                self.id = Some(session_id);
            }
            session_manager::Event::Seat { seat } => {
                self.seat = Some(seat);
            }
            session_manager::Event::Env { name, value } => {
                self.env.push((name, value));
            }
            session_manager::Event::Done => {
                self.done = true;
            }
            session_manager::Event::Error { .. } => {
                self.id = None;
                self.done = true;
            }
        }
    }
}

struct Pam;

impl PamServiceModule for Pam {
    fn open_session(pamh: pamsm::Pam, _: pamsm::PamFlags, _: Vec<String>) -> pamsm::PamError {
        fn inner(pamh: &pamsm::Pam) -> Result<(), pamsm::PamError> {
            let mut session = Session {
                id: None,
                seat: None,
                env: Vec::new(),
                done: false,
            };

            let mut socket = client::Client::connect("/run/sessiond.sock")
                .map_err(|_| pamsm::PamError::SESSION_ERR)?;
            let event_queue = socket.new_event_queue();

            socket.add_implementation::<session_management_v1::SessionManagementV1Impl>();

            event_queue
                .wait_for_handshake(&mut session)
                .map_err(|_| pamsm::PamError::SESSION_ERR)?;

            let manager = socket
                .bind::<session_manager::SessionManager, _>(&event_queue, &mut session, 1)
                .map_err(|_| pamsm::PamError::SESSION_ERR)?;

            let username = pamh
                .get_user(None)?
                .ok_or(pamsm::PamError::USER_UNKNOWN)?
                .to_string_lossy()
                .to_string();
            let uid = uzers::get_user_by_name(&username)
                .ok_or(pamsm::PamError::USER_UNKNOWN)?
                .uid();
            let vt = pamh
                .get_tty()?
                .and_then(|tty| {
                    tty.to_string_lossy()
                        .rsplit('/')
                        .next()
                        .and_then(|name| name.strip_prefix("tty"))
                        .and_then(|vt| vt.parse().ok())
                })
                .unwrap_or(0);

            let remote_host = pamh
                .get_rhost()?
                .map(|rhost| rhost.to_string_lossy().to_string())
                .unwrap_or_default();
            let is_local = u32::from(remote_host.is_empty());

            let envlist = pamh.getenvlist()?;
            for env in envlist.iter() {
                if let Some((name, value)) = env.to_string_lossy().split_once('=') {
                    manager.send_env(name, value);
                }
            }

            manager.send_create_session(uid, vt, is_local, remote_host);

            while !session.done {
                event_queue
                    .dispatch_events(&mut session, true)
                    .map_err(|_| pamsm::PamError::SESSION_ERR)?;
            }

            let session_id = session.id.ok_or(pamsm::PamError::SESSION_ERR)?;
            pamh.send_bytes("sessiond", session_id.to_le_bytes().to_vec(), None)
                .map_err(|_| pamsm::PamError::SESSION_ERR)?;

            for (name, value) in session.env {
                pamh.putenv(&format!("{name}={value}"))?;
            }

            Ok(())
        }

        match inner(&pamh) {
            Ok(()) => pamsm::PamError::SUCCESS,
            Err(e) => e,
        }
    }

    fn close_session(pamh: pamsm::Pam, _: pamsm::PamFlags, _: Vec<String>) -> pamsm::PamError {
        fn inner(pamh: &pamsm::Pam) -> Result<(), pamsm::PamError> {
            let session_id = pamh
                .retrieve_bytes("sessiond")
                .map_err(|_| pamsm::PamError::SESSION_ERR)
                .and_then(|bytes| {
                    bytes
                        .try_into()
                        .map(u32::from_le_bytes)
                        .map_err(|_| pamsm::PamError::SESSION_ERR)
                })?;

            let mut session = Session {
                id: None,
                seat: None,
                env: Vec::new(),
                done: false,
            };

            let mut socket = client::Client::connect("/run/sessiond.sock")
                .map_err(|_| pamsm::PamError::SESSION_ERR)?;
            let event_queue = socket.new_event_queue();

            socket.add_implementation::<session_management_v1::SessionManagementV1Impl>();

            event_queue
                .wait_for_handshake(&mut session)
                .map_err(|_| pamsm::PamError::SESSION_ERR)?;

            let manager = socket
                .bind::<session_manager::SessionManager, _>(&event_queue, &mut session, 1)
                .map_err(|_| pamsm::PamError::SESSION_ERR)?;

            manager.send_destroy_session(session_id);

            Ok(())
        }

        match inner(&pamh) {
            Ok(()) => pamsm::PamError::SUCCESS,
            Err(e) => e,
        }
    }
}

pamsm::pam_module!(Pam);
