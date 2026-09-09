use hyprwire::client;
use hyprwire_protocols_sessiond::client::session_core::v1::session_core_v1;
use hyprwire_protocols_sessiond::client::session_core::v1::session_core_v1::{
    session_core_manager, session_core_session,
};
use hyprwire_protocols_sessiond::client::session_power::v1::session_power_v1;
use hyprwire_protocols_sessiond::client::session_power::v1::session_power_v1::session_power_manager;
use std::process;

pub struct Session {
    id: Option<u32>,
    pid: u32,
    object: session_core_session::SessionCoreSession,
}

pub struct Sessiond {
    eq: hyprwire::EventQueue,
    session_manager: Option<session_core_manager::SessionCoreManager>,
    sessions: Vec<Session>,
    power: Option<Power>,
}

pub struct Power {
    eq: hyprwire::EventQueue,
    manager: session_power_manager::SessionPowerManager,
    done: bool,
    error: Option<String>,
    can_reboot: bool,
    can_poweroff: bool,
    can_suspend: bool,
    can_hibernate: bool,
    can_hybrid_sleep: bool,
}

impl hyprwire::Dispatch<session_core_session::SessionCoreSession> for Sessiond {
    fn event(
        &mut self,
        object: &session_core_session::SessionCoreSession,
        event: <session_core_session::SessionCoreSession as hyprwire::Object>::Event<'_>,
    ) {
        let Some(session) = self
            .sessions
            .iter_mut()
            .find(|session| session.object == *object)
        else {
            return;
        };
        if let session_core_session::Event::SessionId { session_id } = event {
            session.id = Some(session_id);
        }
    }
}

impl hyprwire::Dispatch<session_power_manager::SessionPowerManager> for Sessiond {
    fn event(
        &mut self,
        _object: &session_power_manager::SessionPowerManager,
        event: <session_power_manager::SessionPowerManager as hyprwire::Object>::Event<'_>,
    ) {
        let Some(power) = self.power.as_mut() else {
            return;
        };

        match event {
            session_power_manager::Event::CanReboot { can_reboot } => {
                power.can_reboot = can_reboot != 0;
            }
            session_power_manager::Event::CanPoweroff { can_poweroff } => {
                power.can_poweroff = can_poweroff != 0;
            }
            session_power_manager::Event::CanSuspend { can_suspend } => {
                power.can_suspend = can_suspend != 0;
            }
            session_power_manager::Event::CanHibernate { can_hibernate } => {
                power.can_hibernate = can_hibernate != 0;
            }
            session_power_manager::Event::CanHybridSleep { can_hybrid_sleep } => {
                power.can_hybrid_sleep = can_hybrid_sleep != 0;
            }
            session_power_manager::Event::Done => power.done = true,
            session_power_manager::Event::Error { message, .. } => {
                power.error = Some(message);
                power.done = true;
            }
            _ => {}
        }
    }
}

impl Sessiond {
    pub fn try_connect() -> Option<Self> {
        let Ok(mut socket) = client::Client::connect("/run/sessiond/sessiond.sock") else {
            return None;
        };

        let eq = socket.new_event_queue();

        socket.add_implementation::<session_core_v1::SessionCoreV1>();

        let mut sessiond = Self {
            eq: eq.clone(),
            sessions: Vec::new(),
            session_manager: None,
            power: None,
        };

        eq.wait_for_handshake(&mut sessiond).ok()?;

        // if we fail to bind manager the connectionn is useless, try another backend
        sessiond.session_manager = Some(socket.bind(&eq, &mut sessiond, 3).ok()?);

        eq.roundtrip(&mut sessiond).ok()?;

        if let Ok(mut socket) = client::Client::connect("/run/sessiond/power.sock") {
            let eq = socket.new_event_queue();

            socket.add_implementation::<session_power_v1::SessionPowerV1>();

            eq.wait_for_handshake(&mut sessiond).ok()?;

            if let Ok(power_manager) =
                socket.bind::<session_power_manager::SessionPowerManager, _>(&eq, &mut sessiond, 1)
            {
                sessiond.power = Some(Power {
                    eq: eq.clone(),
                    manager: power_manager,
                    done: false,
                    error: None,
                    can_reboot: false,
                    can_poweroff: false,
                    can_suspend: false,
                    can_hibernate: false,
                    can_hybrid_sleep: false,
                });
                _ = eq.roundtrip(&mut sessiond);
            }
        }

        Some(sessiond)
    }
}

impl super::Backend for Sessiond {
    fn get_session_id(&mut self) -> Option<String> {
        let pid = process::id();
        let eq = self.eq.clone();

        if let Some(index) = self.sessions.iter().position(|session| session.pid == pid) {
            loop {
                if let Some(id) = self.sessions[index].id {
                    return Some(id.to_string());
                }
                eq.dispatch_events(self, true).ok()?;
            }
        }

        let session_manager = self.session_manager.as_ref()?;
        let session_object = session_manager.send_get_current_session::<Self>()?;

        self.sessions.push(Session {
            id: None,
            pid,
            object: session_object.clone(),
        });

        loop {
            eq.dispatch_events(self, true).ok()?;
            if let Some(session) = self
                .sessions
                .iter()
                .find(|session| session.object == session_object)
                && let Some(id) = session.id
            {
                return Some(id.to_string());
            }
        }
    }

    fn can_power_off(&mut self) -> Result<bool, super::Error> {
        self.dispatch_power_events()?;
        self.power
            .as_ref()
            .map(|power| power.can_poweroff)
            .ok_or(super::Error::Unsupported)
    }

    fn power_off(&mut self) -> Result<(), super::Error> {
        let Some(power) = self.power.as_mut() else {
            return Err(super::Error::Unsupported);
        };
        power.done = false;
        power.error = None;
        power.manager.send_poweroff();
        let eq = power.eq.clone();
        while !self.power.as_ref().is_some_and(|power| power.done) {
            eq.dispatch_events(self, true)
                .map_err(|_| super::Error::Failed)?;
        }
        if self
            .power
            .as_ref()
            .is_some_and(|power| power.error.is_some())
        {
            Err(super::Error::Failed)
        } else {
            Ok(())
        }
    }

    fn can_suspend(&mut self) -> Result<bool, super::Error> {
        self.dispatch_power_events()?;
        self.power
            .as_ref()
            .map(|power| power.can_suspend)
            .ok_or(super::Error::Unsupported)
    }

    fn suspend(&mut self) -> Result<(), super::Error> {
        let Some(power) = self.power.as_mut() else {
            return Err(super::Error::Unsupported);
        };
        power.done = false;
        power.error = None;
        power.manager.send_suspend();
        let eq = power.eq.clone();
        while !self.power.as_ref().is_some_and(|power| power.done) {
            eq.dispatch_events(self, true)
                .map_err(|_| super::Error::Failed)?;
        }
        if self
            .power
            .as_ref()
            .is_some_and(|power| power.error.is_some())
        {
            Err(super::Error::Failed)
        } else {
            Ok(())
        }
    }

    fn can_hibernate(&mut self) -> Result<bool, super::Error> {
        self.dispatch_power_events()?;
        self.power
            .as_ref()
            .map(|power| power.can_hibernate)
            .ok_or(super::Error::Unsupported)
    }

    fn hibernate(&mut self) -> Result<(), super::Error> {
        let Some(power) = self.power.as_mut() else {
            return Err(super::Error::Unsupported);
        };
        power.done = false;
        power.error = None;
        power.manager.send_hibernate();
        let eq = power.eq.clone();
        while !self.power.as_ref().is_some_and(|power| power.done) {
            eq.dispatch_events(self, true)
                .map_err(|_| super::Error::Failed)?;
        }
        if self
            .power
            .as_ref()
            .is_some_and(|power| power.error.is_some())
        {
            Err(super::Error::Failed)
        } else {
            Ok(())
        }
    }

    fn can_reboot(&mut self) -> Result<bool, super::Error> {
        self.dispatch_power_events()?;
        self.power
            .as_ref()
            .map(|power| power.can_reboot)
            .ok_or(super::Error::Unsupported)
    }

    fn reboot(&mut self) -> Result<(), super::Error> {
        let Some(power) = self.power.as_mut() else {
            return Err(super::Error::Unsupported);
        };
        power.done = false;
        power.error = None;
        power.manager.send_reboot();
        let eq = power.eq.clone();
        while !self.power.as_ref().is_some_and(|power| power.done) {
            eq.dispatch_events(self, true)
                .map_err(|_| super::Error::Failed)?;
        }
        if self
            .power
            .as_ref()
            .is_some_and(|power| power.error.is_some())
        {
            Err(super::Error::Failed)
        } else {
            Ok(())
        }
    }

    fn hybrid_sleep(&mut self) -> Result<(), super::Error> {
        let Some(power) = self.power.as_mut() else {
            return Err(super::Error::Unsupported);
        };
        power.done = false;
        power.error = None;
        power.manager.send_hybrid_sleep();
        let eq = power.eq.clone();
        while !self.power.as_ref().is_some_and(|power| power.done) {
            eq.dispatch_events(self, true)
                .map_err(|_| super::Error::Failed)?;
        }
        if self
            .power
            .as_ref()
            .is_some_and(|power| power.error.is_some())
        {
            Err(super::Error::Failed)
        } else {
            Ok(())
        }
    }

    fn can_hybrid_sleep(&mut self) -> Result<bool, super::Error> {
        self.dispatch_power_events()?;
        self.power
            .as_ref()
            .map(|power| power.can_hybrid_sleep)
            .ok_or(super::Error::Unsupported)
    }
}

impl Sessiond {
    fn dispatch_power_events(&mut self) -> Result<(), super::Error> {
        let Some(power) = self.power.as_ref() else {
            return Err(super::Error::Unsupported);
        };
        let eq = power.eq.clone();
        eq.roundtrip(self).map_err(|_| super::Error::Failed)
    }
}

hyprwire::delegate_noop!(Sessiond: ignore session_core_manager::SessionCoreManager);
