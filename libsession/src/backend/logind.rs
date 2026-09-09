use zbus::blocking::connection;

#[zbus::proxy(
    default_service = "org.freedesktop.login1",
    interface = "org.freedesktop.login1.Session",
    default_path = "/org/freedesktop/login1/session/auto"
)]
trait LogindSession {
    #[zbus(property)]
    fn id(&self) -> zbus::Result<String>;
}

#[zbus::proxy(
    default_service = "org.freedesktop.login1",
    interface = "org.freedesktop.login1.Manager",
    default_path = "/org/freedesktop/login1"
)]
trait LogindManager {
    #[zbus(name = "PowerOff")]
    fn power_off(&self, interactive: bool) -> zbus::Result<()>;

    #[zbus(name = "Reboot")]
    fn reboot(&self, interactive: bool) -> zbus::Result<()>;

    #[zbus(name = "Suspend")]
    fn suspend(&self, interactive: bool) -> zbus::Result<()>;

    #[zbus(name = "Hibernate")]
    fn hibernate(&self, interactive: bool) -> zbus::Result<()>;

    #[zbus(name = "HybridSleep")]
    fn hybrid_sleep(&self, interactive: bool) -> zbus::Result<()>;

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

pub struct Logind {
    session: LogindSessionProxyBlocking<'static>,
    manager: LogindManagerProxyBlocking<'static>,
}

impl Logind {
    pub fn try_connect() -> Option<Self> {
        let conn = connection::Builder::system().ok()?.build().ok()?;

        let session = LogindSessionProxyBlocking::new(&conn).ok()?;
        let manager = LogindManagerProxyBlocking::new(&conn).ok()?;

        Some(Self { session, manager })
    }
}

impl super::Backend for Logind {
    fn get_session_id(&mut self) -> Option<String> {
        self.session.id().ok()
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
