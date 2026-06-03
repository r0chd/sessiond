mod moxsession_core_v1 {
    hyprwire::include_protocol!("moxsession_core_v1");
    pub use client::*;
}

use hyprwire::client;
use moxsession_core_v1::moxsession_core_manager;
use pamsm::{PamLibExt, PamServiceModule};

struct Session {
    session_id: Option<u32>,
}

impl hyprwire::Dispatch<moxsession_core_manager::MoxsessionCoreManager> for Session {
    fn event(
        &mut self,
        _: &moxsession_core_manager::MoxsessionCoreManager,
        event: <moxsession_core_manager::MoxsessionCoreManager as hyprwire::Object>::Event<'_>,
    ) {
        if let moxsession_core_manager::Event::SessionCreated { session_id } = event {
            self.session_id = Some(session_id);
        }
    }
}

struct Pam;

impl pamsm::PamServiceModule for Pam {
    fn open_session(pamh: pamsm::Pam, _: pamsm::PamFlags, _: Vec<String>) -> pamsm::PamError {
        fn inner(pamh: pamsm::Pam) -> Result<(), pamsm::PamError> {
            let mut session = Session { session_id: None };

            let mut socket = client::Client::connect("/run/mox/moxsession.sock")
                .map_err(|_| pamsm::PamError::SESSION_ERR)?;
            let event_queue = socket.new_event_queue();

            socket.add_implementation::<moxsession_core_v1::MoxsessionCoreV1Impl>();

            event_queue
                .wait_for_handshake(&mut session)
                .map_err(|_| pamsm::PamError::SESSION_ERR)?;

            let manager = socket
                .bind::<moxsession_core_manager::MoxsessionCoreManager, _>(
                    &event_queue,
                    &mut session,
                    1,
                )
                .map_err(|_| pamsm::PamError::SESSION_ERR)?;

            let username = pamh
                .get_user(None)?
                .ok_or(pamsm::PamError::USER_UNKNOWN)?
                .to_string_lossy()
                .to_string();
            let tty = pamh.get_tty()?.ok_or(pamsm::PamError::SESSION_ERR)?;
            let uid = users::get_user_by_name(&username)
                .ok_or(pamsm::PamError::USER_UNKNOWN)?
                .uid();

            manager.send_create_session(uid, username, tty.to_string_lossy());

            while session.session_id.is_none() {
                event_queue
                    .dispatch_events(&mut session, true)
                    .map_err(|_| pamsm::PamError::SESSION_ERR)?;
            }

            let session_id = session.session_id.unwrap();
            pamh.send_bytes("moxsession", session_id.to_le_bytes().to_vec(), None)
                .map_err(|_| pamsm::PamError::SESSION_ERR)?;

            Ok(())
        }

        match inner(pamh) {
            Ok(()) => pamsm::PamError::SUCCESS,
            Err(e) => e,
        }
    }

    fn close_session(pamh: pamsm::Pam, _: pamsm::PamFlags, _: Vec<String>) -> pamsm::PamError {
        fn inner(pamh: pamsm::Pam) -> Result<(), pamsm::PamError> {
            let session_id = pamh
                .retrieve_bytes("moxsession")
                .map_err(|_| pamsm::PamError::SESSION_ERR)
                .and_then(|bytes| {
                    bytes
                        .try_into()
                        .map(u32::from_le_bytes)
                        .map_err(|_| pamsm::PamError::SESSION_ERR)
                })?;

            let mut session = Session { session_id: None };

            let mut socket = client::Client::connect("/run/mox/moxsession.sock")
                .map_err(|_| pamsm::PamError::SESSION_ERR)?;
            let event_queue = socket.new_event_queue();

            socket.add_implementation::<moxsession_core_v1::MoxsessionCoreV1Impl>();

            event_queue
                .wait_for_handshake(&mut session)
                .map_err(|_| pamsm::PamError::SESSION_ERR)?;

            let manager = socket
                .bind::<moxsession_core_manager::MoxsessionCoreManager, _>(
                    &event_queue,
                    &mut session,
                    1,
                )
                .map_err(|_| pamsm::PamError::SESSION_ERR)?;

            manager.send_destroy_session(session_id);

            Ok(())
        }

        match inner(pamh) {
            Ok(()) => pamsm::PamError::SUCCESS,
            Err(e) => e,
        }
    }
}

pamsm::pam_module!(Pam);
