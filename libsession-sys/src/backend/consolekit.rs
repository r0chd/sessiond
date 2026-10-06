use crate::logging;
use std::{ffi, io, path, rc};
use zbus::{blocking, zvariant};

#[zbus::proxy(
    default_service = "org.freedesktop.ConsoleKit",
    interface = "org.freedesktop.ConsoleKit.Manager",
    default_path = "/org/freedesktop/ConsoleKit/Manager"
)]
trait ConsoleKitManager {
    #[zbus(name = "GetSessions")]
    fn get_sessions(&self) -> zbus::Result<Vec<zvariant::OwnedObjectPath>>;
    #[zbus(name = "GetSessionForUnixProcess")]
    fn get_session_for_unix_process(&self, pid: u32) -> zbus::Result<zvariant::OwnedObjectPath>;
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

    #[zbus(name = "IsLocal")]
    fn is_local(&self) -> zbus::Result<bool>;
}

pub struct ConsoleKit {
    logger: rc::Rc<logging::Logger>,
    connection: blocking::Connection,
    manager: ConsoleKitManagerProxyBlocking<'static>,
}

impl ConsoleKit {
    pub fn try_connect(logger: rc::Rc<logging::Logger>) -> Option<Self> {
        let connection = blocking::connection::Builder::system().ok()?.build().ok()?;

        let manager = ConsoleKitManagerProxyBlocking::new(&connection).ok()?;

        Some(Self {
            logger,
            connection,
            manager,
        })
    }
}

impl super::Backend for ConsoleKit {
    fn get_monitor(&self) -> Option<Box<dyn super::BackendMonitor>> {
        Some(Box::new(super::inotify::InotifyMonitor::new(
            path::Path::new("/var/run/ConsoleKit"),
        )?))
    }

    fn get_owner_uid(&mut self, pid: u32) -> super::Result<u32> {
        let path = self.manager.get_session_for_unix_process(pid)?;
        let session = ConsoleKitSessionProxyBlocking::builder(&self.connection)
            .path(path)?
            .build()?;
        session.get_unix_user().map_err(super::Error::Dbus)
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
    ) -> super::Result<crate::LibsessionSessionState> {
        let session_id = session_id.to_string_lossy();
        for path in self.manager.get_sessions()? {
            let session = ConsoleKitSessionProxyBlocking::builder(&self.connection)
                .path(path)?
                .build()?;
            if session.get_id()?.as_str() == session_id {
                return Ok(if session.is_active()? {
                    crate::LibsessionSessionState::Active
                } else {
                    crate::LibsessionSessionState::Online
                });
            }
        }

        Err(io::Error::new(io::ErrorKind::NotFound, "session not found").into())
    }

    fn is_local_session(&self, session_id: &ffi::CStr) -> Option<bool> {
        let session_id = session_id.to_string_lossy();
        for path in self.manager.get_sessions().ok()? {
            let session = ConsoleKitSessionProxyBlocking::builder(&self.connection)
                .path(path)
                .ok()?
                .build()
                .ok()?;
            if session.get_id().ok()?.as_str() == session_id {
                return session.is_local().ok();
            }
        }
        None
    }

    fn get_session_id(&mut self, pid: u32) -> Option<String> {
        self.logger.log(crate::LibsessionLogLevel::Debug, || {
            format!("consolekit: looking up session for process {pid}")
        });
        let session_path = self.manager.get_session_for_unix_process(pid).ok()?;

        let session = ConsoleKitSessionProxyBlocking::builder(&self.connection)
            .path(session_path)
            .ok()?
            .build()
            .ok()?;

        session.get_id().ok()
    }
}
