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

pub struct Logind {
    session: LogindSessionProxyBlocking<'static>,
}

impl Logind {
    pub fn try_connect() -> Option<Self> {
        let conn = connection::Builder::system().ok()?.build().ok()?;

        let session = LogindSessionProxyBlocking::new(&conn).ok()?;

        Some(Self { session })
    }
}

impl super::Backend for Logind {
    fn get_session_id(&mut self) -> Option<String> {
        self.session.id().ok()
    }
}
