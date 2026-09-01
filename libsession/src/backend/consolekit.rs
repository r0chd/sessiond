use zbus::blocking::{Connection, connection};
use zbus::zvariant::OwnedObjectPath;

#[zbus::proxy(
    default_service = "org.freedesktop.ConsoleKit",
    interface = "org.freedesktop.ConsoleKit.Manager",
    default_path = "/org/freedesktop/ConsoleKit/Manager"
)]
trait ConsoleKitManager {
    #[zbus(name = "GetCurrentSession")]
    fn get_current_session(&self) -> zbus::Result<OwnedObjectPath>;
}

#[zbus::proxy(
    default_service = "org.freedesktop.ConsoleKit",
    interface = "org.freedesktop.ConsoleKit.Session"
)]
trait ConsoleKitSession {
    #[zbus(name = "GetId")]
    fn get_id(&self) -> zbus::Result<String>;
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
    fn get_session_id(&mut self) -> Option<String> {
        let session_path = self.manager.get_current_session().ok()?;

        let session = ConsoleKitSessionProxyBlocking::builder(&self.connection)
            .path(session_path)
            .ok()?
            .build()
            .ok()?;

        session.get_id().ok()
    }
}
