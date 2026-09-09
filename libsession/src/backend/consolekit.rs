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

    #[zbus(name = "PowerOff")]
    fn power_off(&self, policykit_interactivity: bool) -> zbus::Result<()>;

    #[zbus(name = "Reboot")]
    fn reboot(&self, policykit_interactivity: bool) -> zbus::Result<()>;

    #[zbus(name = "Suspend")]
    fn suspend(&self, policykit_interactivity: bool) -> zbus::Result<()>;

    #[zbus(name = "Hibernate")]
    fn hibernate(&self, policykit_interactivity: bool) -> zbus::Result<()>;

    #[zbus(name = "HybridSleep")]
    fn hybrid_sleep(&self, policykit_interactivity: bool) -> zbus::Result<()>;

    #[zbus(name = "CanPowerOff")]
    fn can_power_off(&self) -> zbus::Result<String>;

    #[zbus(name = "CanReboot")]
    fn can_reboot(&self) -> zbus::Result<String>;

    #[zbus(name = "CanSuspend")]
    fn can_suspend(&self) -> zbus::Result<String>;

    #[zbus(name = "CanHibernate")]
    fn can_hibernate(&self) -> zbus::Result<String>;

    #[zbus(name = "CanHybridSleep")]
    fn can_hybrid_sleep(&self) -> zbus::Result<String>;
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

    fn power_off(&mut self) -> Result<(), super::Error> {
        self.manager
            .power_off(true)
            .map_err(|_| super::Error::Failed)
    }

    fn can_power_off(&mut self) -> Result<bool, super::Error> {
        self.manager
            .can_power_off()
            .map(|result| result == "yes" || result == "challenge")
            .map_err(|_| super::Error::Failed)
    }

    fn reboot(&mut self) -> Result<(), super::Error> {
        self.manager.reboot(true).map_err(|_| super::Error::Failed)
    }

    fn can_reboot(&mut self) -> Result<bool, super::Error> {
        self.manager
            .can_reboot()
            .map(|result| result == "yes" || result == "challenge")
            .map_err(|_| super::Error::Failed)
    }

    fn suspend(&mut self) -> Result<(), super::Error> {
        self.manager.suspend(true).map_err(|_| super::Error::Failed)
    }

    fn can_suspend(&mut self) -> Result<bool, super::Error> {
        self.manager
            .can_suspend()
            .map(|result| result == "yes" || result == "challenge")
            .map_err(|_| super::Error::Failed)
    }

    fn hibernate(&mut self) -> Result<(), super::Error> {
        self.manager
            .hibernate(true)
            .map_err(|_| super::Error::Failed)
    }

    fn can_hibernate(&mut self) -> Result<bool, super::Error> {
        self.manager
            .can_hibernate()
            .map(|result| result == "yes" || result == "challenge")
            .map_err(|_| super::Error::Failed)
    }

    fn hybrid_sleep(&mut self) -> Result<(), super::Error> {
        self.manager
            .hybrid_sleep(true)
            .map_err(|_| super::Error::Failed)
    }

    fn can_hybrid_sleep(&mut self) -> Result<bool, super::Error> {
        self.manager
            .can_hybrid_sleep()
            .map(|result| result == "yes" || result == "challenge")
            .map_err(|_| super::Error::Failed)
    }
}
