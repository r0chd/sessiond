use std::ffi;
use zbus::blocking::{Connection, connection};
use zbus::zvariant::OwnedObjectPath;

#[zbus::proxy(
    default_service = "org.freedesktop.ConsoleKit",
    interface = "org.freedesktop.ConsoleKit.Manager",
    default_path = "/org/freedesktop/ConsoleKit/Manager"
)]
trait ConsoleKitManager {
    #[zbus(name = "GetSessions")]
    fn get_sessions(&self) -> zbus::Result<Vec<OwnedObjectPath>>;
    #[zbus(name = "GetSessionForUnixProcess")]
    fn get_session_for_unix_process(&self, pid: u32) -> zbus::Result<OwnedObjectPath>;
}

#[zbus::proxy(
    default_service = "org.freedesktop.ConsoleKit",
    interface = "org.freedesktop.ConsoleKit.Session"
)]
trait ConsoleKitSession {
    #[zbus(name = "GetId")]
    fn get_id(&self) -> zbus::Result<String>;

    #[zbus(name = "GetUnixUser")]
    fn get_unix_user(&self) -> zbus::Result<u32>;

    #[zbus(name = "IsActive")]
    fn is_active(&self) -> zbus::Result<bool>;
}

pub struct ConsoleKit {
    connection: Connection,
    manager: ConsoleKitManagerProxyBlocking<'static>,
}

impl ConsoleKit {
    pub fn try_connect() -> Option<Self> {
        let connection = connection::Builder::system().ok()?.build().ok()?;

        let manager = ConsoleKitManagerProxyBlocking::new(&connection).ok()?;

        Some(Self {
            connection,
            manager,
        })
    }
}

impl super::Backend for ConsoleKit {
    fn get_owner_uid(&mut self, pid: u32) -> Option<u32> {
        let path = self.manager.get_session_for_unix_process(pid).ok()?;
        let session = ConsoleKitSessionProxyBlocking::builder(&self.connection)
            .path(path)
            .ok()?
            .build()
            .ok()?;
        session.get_unix_user().ok()
    }

    fn get_state(&mut self, uid: u32) -> Option<crate::LibsessionUserState> {
        let mut found = false;
        let mut active = false;

        for path in self.manager.get_sessions().ok()? {
            let session = ConsoleKitSessionProxyBlocking::builder(&self.connection)
                .path(path)
                .ok()?
                .build()
                .ok()?;
            if session.get_unix_user().ok()? != uid {
                continue;
            }
            found = true;
            active |= session.is_active().ok()?;
        }

        Some(if active {
            crate::LibsessionUserState::Active
        } else if found {
            crate::LibsessionUserState::Online
        } else {
            crate::LibsessionUserState::Offline
        })
    }

    fn get_display(&mut self, uid: u32) -> Option<String> {
        for path in self.manager.get_sessions().ok()? {
            let session = ConsoleKitSessionProxyBlocking::builder(&self.connection)
                .path(path)
                .ok()?
                .build()
                .ok()?;
            if session.get_unix_user().ok()? == uid && session.is_active().ok()? {
                return session.get_id().ok();
            }
        }
        None
    }

    fn get_uid(&mut self, session_id: &ffi::CStr) -> Option<u32> {
        let session_id = session_id.to_string_lossy();
        for path in self.manager.get_sessions().ok()? {
            let session = ConsoleKitSessionProxyBlocking::builder(&self.connection)
                .path(path)
                .ok()?
                .build()
                .ok()?;
            if session.get_id().ok()?.as_str() == session_id {
                return session.get_unix_user().ok();
            }
        }
        None
    }

    fn get_session_state(
        &mut self,
        session_id: &ffi::CStr,
    ) -> Option<crate::LibsessionSessionState> {
        let session_id = session_id.to_string_lossy();
        for path in self.manager.get_sessions().ok()? {
            let session = ConsoleKitSessionProxyBlocking::builder(&self.connection)
                .path(path)
                .ok()?
                .build()
                .ok()?;
            if session.get_id().ok()?.as_str() == session_id {
                return Some(if session.is_active().ok()? {
                    crate::LibsessionSessionState::Active
                } else {
                    crate::LibsessionSessionState::Online
                });
            }
        }
        None
    }

    fn get_session_id(&mut self, pid: u32) -> Option<String> {
        let session_path = self.manager.get_session_for_unix_process(pid).ok()?;

        let session = ConsoleKitSessionProxyBlocking::builder(&self.connection)
            .path(session_path)
            .ok()?
            .build()
            .ok()?;

        session.get_id().ok()
    }
}
