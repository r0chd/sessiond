use crate::seat_v1::{seat, seat_manager};
use crate::{client, device, seat_v1, session, terminal};
use rustix::io;
use std::os::fd::AsFd;
use std::{cell, fs, rc};

const MAX_SEAT_DEVICES: u8 = 128;

impl seat_v1::SeatV1Handler for crate::Sessiond {
    fn bind(&mut self, object: seat_manager::SeatManager) {
        let Some(client) = object.client() else {
            object.error(
                io::Errno::PERM.raw_os_error().cast_unsigned(),
                "seat access requires peer credentials",
            );
            return;
        };

        let creds = client.creds();
        if self.seat_access.allows(creds) {
            return;
        }

        log::warn!(
            pid = creds.pid.as_raw_pid(),
            uid = creds.uid.as_raw(),
            gid = creds.gid.as_raw();
            "rejected seat_v1 bind from unauthorized client"
        );
        object.error(
            io::Errno::PERM.raw_os_error().cast_unsigned(),
            "seat access denied",
        );
    }
}

impl hyprwire::Dispatch<seat_manager::SeatManager> for crate::Sessiond {
    fn event(
        &mut self,
        object: &seat_manager::SeatManager,
        event: <seat_manager::SeatManager as hyprwire::Object>::Event<'_>,
    ) {
        match event {
            seat_manager::Event::OpenSeat { seat } => {
                let Some(server_client) = object.client() else {
                    seat.error(
                        io::Errno::PERM.raw_os_error().cast_unsigned(),
                        "seat access requires peer credentials",
                    );
                    return;
                };

                let creds = server_client.creds();
                if !self.seat_access.allows(creds) {
                    log::warn!(
                        pid = creds.pid.as_raw_pid(),
                        uid = creds.uid.as_raw(),
                        gid = creds.gid.as_raw();
                        "rejected seat open from unauthorized client"
                    );
                    seat.error(
                        io::Errno::PERM.raw_os_error().cast_unsigned(),
                        "seat access denied",
                    );
                    return;
                }

                let session_id = {
                    let pid = server_client.creds().pid.as_raw_pid().cast_unsigned();
                    match self.cgroup_manager.find_session_by_pid(pid) {
                        Ok(session_id) => session_id,
                        Err(err) => {
                            log::error!("Could not find session for seat opener {pid}: {err}");
                            None
                        }
                    }
                };

                let client = rc::Rc::new(cell::RefCell::new(client::Client {
                    seat_name: String::new(),
                    object: seat,
                    devices: Vec::new(),
                    state: crate::seat::SeatState::New,
                    session_id: None,
                }));

                let Some(seat) = self.seat.first_mut() else {
                    log::error!("No seats configured");
                    client.borrow().object.send_disabled();
                    return;
                };

                if let Err(err) = seat.add_client(rc::Rc::clone(&client)) {
                    log::error!("Could not add client to seat: {err}");
                    client.borrow().object.send_disabled();
                    return;
                }
                if let Err(err) = seat.open_client(&client) {
                    log::error!("Could not open client on seat: {err}");
                    let _ = seat.remove_client(&client);
                    client.borrow().object.send_disabled();
                    return;
                }

                if let Some(session_id) = session_id {
                    let seat_name = client.borrow().seat_name.clone();
                    if let Some(session) = self
                        .sessions
                        .iter_mut()
                        .find(|session| session.id() == session_id)
                    {
                        session.set_seat(seat_name);
                    } else {
                        log::warn!(session_id = session_id.as_raw(); "seat opened for unknown session");
                    }
                }

                self.clients.push(client);
            }
            seat_manager::Event::Destroy => {}
        }
    }
}

impl hyprwire::Dispatch<seat::Seat> for crate::Sessiond {
    #[allow(clippy::too_many_lines)]
    fn event(&mut self, object: &seat::Seat, event: <seat::Seat as hyprwire::Object>::Event<'_>) {
        let Some(client) = self
            .clients
            .iter()
            .find(|client| client.borrow().object == *object)
            .cloned()
        else {
            return;
        };

        let Some(seat) = self.seat.first_mut() else {
            log::error!("No seats configured");
            return;
        };

        match event {
            seat::Event::AckDisabled => {
                let state = client.borrow().state;
                if let Err(err) = seat.ack_disable_client(&client) {
                    log::error!("Could not ack disable: {err}");

                    match state {
                        SeatState::Enabled => object.send_seat_error(
                            seat_v1::SeatError::InvalidAckDisabled,
                            "ack_disabled was sent while enabled",
                        ),
                        SeatState::Disabled => object.send_seat_error(
                            seat_v1::SeatError::AlreadyDisabled,
                            "disabled state was already acknowledged",
                        ),
                        _ => {}
                    }
                }
            }
            seat::Event::OpenDevice { device, path } => {
                let seat_name = client.borrow().seat_name.clone();
                let Some(seat) = self.seat.iter().find(|seat| seat.name() == seat_name) else {
                    log::error!("No shared seat found for {seat_name}");
                    return;
                };

                if !seat.is_active_client(&client) {
                    device.send_failed(
                        seat_v1::DeviceError::SeatDisabled,
                        io::Errno::PERM.raw_os_error().cast_unsigned(),
                        "open_device is only valid for the active seat client",
                    );
                    return;
                }

                let mut client = client.borrow_mut();

                let Ok(sanitized_path) = fs::canonicalize(path) else {
                    device.send_failed(
                        seat_v1::DeviceError::OpenFailed,
                        io::Errno::NOENT.raw_os_error().cast_unsigned(),
                        "could not canonicalize device path",
                    );
                    return;
                };

                if client.state != crate::seat::SeatState::Enabled {
                    device.send_failed(
                        seat_v1::DeviceError::SeatDisabled,
                        io::Errno::BUSY.raw_os_error().cast_unsigned(),
                        "open_device is only valid while the seat is enabled",
                    );
                    return;
                }

                let Some(kind) = crate::device::DeviceKind::from_path(&sanitized_path) else {
                    log::error!(
                        "{} is not a supported device type ",
                        sanitized_path.display()
                    );
                    device.send_failed(
                        seat_v1::DeviceError::OpenFailed,
                        io::Errno::INVAL.raw_os_error().cast_unsigned(),
                        "unsupported device type",
                    );
                    return;
                };

                if let Some(old_device) = client
                    .devices
                    .iter_mut()
                    .find(|device| device.matches_path(&sanitized_path))
                {
                    old_device.add_object(device);
                    if let Some(object) = old_device.last_object() {
                        object.send_opened(
                            old_device.id(),
                            old_device.fd().as_fd(),
                            old_device.absolute_path_string(),
                        );
                    }
                    return;
                }

                if client.devices.len() >= usize::from(MAX_SEAT_DEVICES) {
                    log::error!("Client exceeded max seat devices");
                    device.send_failed(
                        seat_v1::DeviceError::OpenFailed,
                        io::Errno::MFILE.raw_os_error().cast_unsigned(),
                        "client exceeded max seat devices",
                    );
                    return;
                }

                let device_id = client
                    .devices
                    .iter()
                    .map(device::Device::id)
                    .max()
                    .map_or(1, |id| id + 1);

                let fd = match rustix::fs::open(
                    &sanitized_path,
                    rustix::fs::OFlags::RDWR
                        | rustix::fs::OFlags::NOCTTY
                        | rustix::fs::OFlags::NOFOLLOW
                        | rustix::fs::OFlags::CLOEXEC
                        | rustix::fs::OFlags::NONBLOCK,
                    rustix::fs::Mode::empty(),
                ) {
                    Ok(fd) => fd,
                    Err(e) => {
                        log::error!("Could not open file: {e}");
                        device.send_failed(
                            seat_v1::DeviceError::OpenFailed,
                            e.raw_os_error().cast_unsigned(),
                            "could not open device",
                        );
                        return;
                    }
                };

                let mut seat_device = device::Device::new(
                    device,
                    sanitized_path,
                    kind,
                    device_id,
                    matches!(kind, device::DeviceKind::Evdev | device::DeviceKind::Hidraw),
                    fd,
                );

                if let Err(err) = seat_device.activate() {
                    log::error!("Could not activate {}: {err}", seat_device.path().display());
                    let errno = err
                        .root_cause()
                        .downcast_ref::<io::Errno>()
                        .copied()
                        .unwrap_or(io::Errno::INVAL);
                    if let Some(object) = seat_device.last_object() {
                        object.send_failed(
                            seat_v1::DeviceError::OpenFailed,
                            errno.raw_os_error().cast_unsigned(),
                            "could not activate device",
                        );
                    }
                    return;
                }

                if let Some(object) = seat_device.last_object() {
                    object.send_opened(
                        device_id,
                        seat_device.fd().as_fd(),
                        seat_device.absolute_path_string(),
                    );
                }
                client.devices.push(seat_device);
            }
            seat::Event::SwitchSession { session } => {
                if !seat.is_vt_bound() {
                    object.send_seat_error(
                        seat_v1::SeatError::InvalidSwitchSession,
                        "session switching is not supported",
                    );
                    return;
                }

                if let Err(err) = seat.set_next_session(&client, session) {
                    log::error!("Could not switch session: {err}");
                }
            }
            seat::Event::Destroy => {
                if let Err(err) = seat.remove_client(&client) {
                    log::error!("Could not remove client from seat: {err}");
                }
                self.clients.retain(|c| !rc::Rc::ptr_eq(c, &client));
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SeatState {
    New,
    Enabled,
    PendingDisabled,
    Disabled,
    Closed,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SeatKind {
    VtBound,
    NonVtBound,
}

pub struct Seat {
    name: String,
    clients: Vec<rc::Rc<cell::RefCell<client::Client>>>,
    active_client: Option<rc::Rc<cell::RefCell<client::Client>>>,
    next_client: Option<rc::Rc<cell::RefCell<client::Client>>>,
    kind: SeatKind,
    cur_vt: u32,
}

impl Default for Seat {
    fn default() -> Self {
        Self {
            name: "seat0".to_string(),
            clients: Vec::new(),
            active_client: None,
            next_client: None,
            kind: SeatKind::NonVtBound,
            cur_vt: 0,
        }
    }
}

impl Seat {
    pub fn vt_bound(name: String) -> Self {
        log::info!("Created VT-bound seat {name}");
        Self {
            name,
            kind: SeatKind::VtBound,
            ..Default::default()
        }
    }

    pub fn non_vt_bound(name: String) -> Self {
        Self {
            name,
            ..Default::default()
        }
    }

    pub fn is_vt_bound(&self) -> bool {
        self.kind == SeatKind::VtBound
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn is_active_client(&self, client: &rc::Rc<cell::RefCell<client::Client>>) -> bool {
        self.active_client
            .as_ref()
            .is_some_and(|active_client| rc::Rc::ptr_eq(active_client, client))
    }

    pub fn is_vt_active(&mut self, vt: u32) -> anyhow::Result<bool> {
        if !self.is_vt_bound() {
            return Ok(false);
        }

        self.update_vt()?;
        Ok(self.cur_vt == vt)
    }

    pub fn active_vt(&mut self) -> anyhow::Result<Option<u32>> {
        if !self.is_vt_bound() {
            return Ok(None);
        }

        self.update_vt()?;
        Ok(Some(self.cur_vt))
    }

    pub fn update_vt(&mut self) -> anyhow::Result<()> {
        let tty0fd = terminal::Terminal::open(0)?;
        self.cur_vt = u32::from(tty0fd.current_vt()?);

        Ok(())
    }

    pub fn vt_open(vt: u32) -> anyhow::Result<()> {
        let terminal = terminal::Terminal::open(vt)?;
        terminal.set_process_switching(true)?;
        terminal.set_keyboard(false)?;
        terminal.set_graphics(true)?;

        Ok(())
    }

    pub fn vt_close(vt: u32) -> anyhow::Result<()> {
        let terminal = terminal::Terminal::open(vt)?;
        terminal.set_process_switching(false)?;
        terminal.set_keyboard(true)?;
        terminal.set_graphics(false)?;

        Ok(())
    }

    pub fn vt_switch(&self, vt: u32) -> anyhow::Result<()> {
        let terminal = terminal::Terminal::open(self.cur_vt)?;
        terminal.set_process_switching(true)?;
        terminal.switch_vt(vt)?;

        Ok(())
    }

    pub fn vt_ack(&self, release: bool) -> anyhow::Result<()> {
        let terminal = terminal::Terminal::open(self.cur_vt)?;

        if release {
            terminal.ack_release()?;
        } else {
            terminal.ack_acquire()?;
        }

        Ok(())
    }

    pub fn vt_activate(&mut self) -> anyhow::Result<u32> {
        if !self.is_vt_bound() {
            log::debug!("VT activation on non VT-bound seat, ignoring");
            return Err(anyhow::anyhow!("seat is not VT-bound"));
        }

        self.update_vt()?;
        let acquired_vt = self.cur_vt;
        log::debug!("Activating VT");
        self.vt_ack(false)?;

        if self.active_client.is_none() {
            self.activate()?;
        }

        Ok(acquired_vt)
    }

    pub fn vt_release(&mut self) -> anyhow::Result<u32> {
        if !self.is_vt_bound() {
            log::debug!("VT release request on non VT-bound seat, ignoring");
            return Err(anyhow::anyhow!("seat is not VT-bound"));
        }

        self.update_vt()?;
        let released_vt = self
            .active_client
            .as_ref()
            .and_then(|client| client.borrow().session_id)
            .map_or(self.cur_vt, session::SessionId::as_raw);
        log::debug!("Releasing VT");

        if let Some(active_client) = self.active_client.clone() {
            self.disable_client(&active_client)?;
        }

        self.vt_ack(true)?;
        self.cur_vt = 0;

        Ok(released_vt)
    }

    pub fn activate(&mut self) -> anyhow::Result<()> {
        if self.active_client.is_some() {
            return Ok(());
        }

        let next_client = if self.next_client.is_some() {
            log::debug!("Activating next queued client on {}", self.name());
            self.next_client.take()
        } else if self.clients.is_empty() {
            log::info!("No clients on {} to activate", self.name());
            return Err(anyhow::anyhow!("No clients on {} to activate", self.name()));
        } else if self.is_vt_bound() {
            if let Some(client) = self.clients.iter().find(|client| {
                client
                    .borrow()
                    .session_id
                    .is_some_and(|session_id| session_id.as_raw() == self.cur_vt)
            }) {
                log::debug!("Activating client belonging to VT {}", self.cur_vt);
                Some(rc::Rc::clone(client))
            } else {
                log::info!("No clients belonging to VT {} to activate", self.cur_vt);
                return Err(anyhow::anyhow!(
                    "No clients belonging to VT {} to activate",
                    self.cur_vt
                ));
            }
        } else {
            log::debug!("Activating first client on {}", self.name());
            self.clients.first().cloned()
        };

        if let Some(client) = next_client {
            self.open_client(&client)?;
        }

        Ok(())
    }

    pub fn open_client(
        &mut self,
        client: &rc::Rc<cell::RefCell<client::Client>>,
    ) -> anyhow::Result<()> {
        debug_assert_eq!(client.borrow().seat_name, self.name);

        let state = client.borrow().state;
        if state != SeatState::New && state != SeatState::Disabled {
            log::error!("Could not enable client: client is not new or disabled");
            return Err(anyhow::anyhow!("client is not new or disabled"));
        }

        if self.active_client.as_ref().is_some_and(|active_client| {
            !self.is_vt_bound() || active_client.borrow().state != SeatState::PendingDisabled
        }) {
            log::error!("Could not enable client: seat already has an active client");
            return Err(anyhow::anyhow!("seat already has an active client"));
        }

        let session_id = client.borrow().session_id()?;
        if self.is_vt_bound() {
            if let Err(err) = Self::vt_open(session_id.as_raw()) {
                log::error!("Could not open VT for client: {err}");
                return Err(err);
            }
        }

        {
            let mut client_ref = client.borrow_mut();
            for device in &mut client_ref.devices {
                if let Err(err) = device.activate() {
                    log::error!("Could not activate {}: {err}", device.path().display());
                }
            }
        }

        client.borrow().object.send_enabled();

        client.borrow_mut().state = SeatState::Enabled;
        self.active_client = Some(rc::Rc::clone(client));
        log::info!("Opened client {} on {}", session_id.as_raw(), self.name);

        Ok(())
    }

    pub fn add_client(
        &mut self,
        client: rc::Rc<cell::RefCell<client::Client>>,
    ) -> anyhow::Result<()> {
        if !client.borrow().seat_name.is_empty() {
            log::error!("Could not add client: client is already a member of a seat");
            return Err(anyhow::anyhow!("client is already a member of a seat"));
        }

        if self.is_vt_bound()
            && let Some(active_client) = &self.active_client
            && active_client.borrow().state != SeatState::PendingDisabled
        {
            log::error!("Could not add client: seat is VT-bound and has an active client");
            return Err(anyhow::anyhow!("seat is VT-bound and has an active client"));
        }

        if client.borrow().session_id.is_some() {
            log::error!("Could not add client: client cannot be reused");
            return Err(anyhow::anyhow!("client cannot be reused"));
        }

        if self.is_vt_bound() {
            self.update_vt()?;
            if self.cur_vt == 0 {
                log::error!("Could not determine VT for client");
                return Err(anyhow::anyhow!("could not determine VT for client"));
            }

            if self.active_client.is_some()
                && self.clients.iter().any(|existing| {
                    existing
                        .borrow()
                        .session_id
                        .is_some_and(|session_id| session_id.as_raw() == self.cur_vt)
                })
            {
                log::error!(
                    "Could not add client: seat is VT-bound and already has pending client"
                );
                return Err(anyhow::anyhow!(
                    "seat is VT-bound and already has pending client"
                ));
            }

            client.borrow_mut().session_id = Some(session::SessionId::from_raw(self.cur_vt));
        } else {
            let mut next_session = 1;
            while self.clients.iter().any(|existing| {
                existing
                    .borrow()
                    .session_id
                    .is_some_and(|session_id| session_id.as_raw() == next_session)
            }) {
                next_session += 1;
            }

            client.borrow_mut().session_id = Some(session::SessionId::from_raw(next_session));
        }

        let session_id = client.borrow().session_id()?;
        client.borrow_mut().seat_name.clone_from(&self.name);
        log::info!("Added client {} to {}", session_id.as_raw(), self.name);
        self.clients.push(client);

        Ok(())
    }

    pub fn disable_client(
        &self,
        client: &rc::Rc<cell::RefCell<client::Client>>,
    ) -> anyhow::Result<()> {
        if client.borrow().state != SeatState::Enabled {
            log::error!("Could not disable client: client is not active");
            return Err(anyhow::anyhow!("client is not active"));
        }

        if self
            .active_client
            .as_ref()
            .is_none_or(|active_client| !rc::Rc::ptr_eq(active_client, client))
        {
            log::error!("Could not disable client: client is not the active seat client");
            return Err(anyhow::anyhow!("client is not the active seat client"));
        }

        {
            let mut client_ref = client.borrow_mut();
            client_ref.devices.retain_mut(|device| {
                if let Err(err) = device.deactivate() {
                    log::error!("Could not deactivate {}: {err}", device.path().display());
                    return true;
                }

                match device.kind() {
                    device::DeviceKind::Drm | device::DeviceKind::Wscons => true,
                    device::DeviceKind::Evdev | device::DeviceKind::Hidraw => {
                        device.send_revoked();
                        false
                    }
                }
            });

            client_ref.state = SeatState::PendingDisabled;
            client_ref.object.send_disabled();
            log::info!(
                "Disabling client {} on {}",
                client_ref.session_id()?.as_raw(),
                self.name
            );
        }

        Ok(())
    }

    pub fn ack_disable_client(
        &mut self,
        client: &rc::Rc<cell::RefCell<client::Client>>,
    ) -> anyhow::Result<()> {
        if client.borrow().state != SeatState::PendingDisabled {
            log::error!("Could not ack disable: client is not pending disable");
            return Err(anyhow::anyhow!("client is not pending disable"));
        }

        client.borrow_mut().state = SeatState::Disabled;
        log::info!(
            "Disabled client {} on {}",
            client.borrow().session_id()?.as_raw(),
            self.name
        );

        if self
            .active_client
            .as_ref()
            .is_none_or(|active_client| !rc::Rc::ptr_eq(active_client, client))
        {
            return Ok(());
        }

        self.active_client = None;
        let _ = self.activate();

        Ok(())
    }

    pub fn set_next_session(
        &mut self,
        client: &rc::Rc<cell::RefCell<client::Client>>,
        session: u32,
    ) -> anyhow::Result<()> {
        if client.borrow().state != SeatState::Enabled {
            log::error!("Could not set next session: client is not active");
            return Err(anyhow::anyhow!("client is not active"));
        }

        if self
            .active_client
            .as_ref()
            .is_none_or(|active_client| !rc::Rc::ptr_eq(active_client, client))
        {
            log::error!("Could not set next session: client is not the active seat client");
            return Err(anyhow::anyhow!("client is not the active seat client"));
        }

        if session == 0 {
            log::error!("Could not set next session: invalid session value {session}");
            return Err(anyhow::anyhow!("invalid session value"));
        }
        let session = session::SessionId::from_raw(session);

        if Some(session) == client.borrow().session_id {
            log::info!("Could not set next session: requested session is already active");
            return Ok(());
        }

        if self.next_client.is_some() {
            log::info!("Could not set next session: switch is already queued");
            return Ok(());
        }

        if self.is_vt_bound() {
            log::info!(
                "Switching from VT {} to VT {}",
                self.cur_vt,
                session.as_raw()
            );
            if let Err(err) = self.vt_switch(session.as_raw()) {
                log::error!("Could not switch VT: {err}");
                return Err(err);
            }
            return Ok(());
        }

        let Some(target) = self
            .clients
            .iter()
            .find(|candidate| candidate.borrow().session_id == Some(session))
            .cloned()
        else {
            log::error!("Could not set next session: no such client");
            return Err(anyhow::anyhow!("no such client"));
        };

        log::info!(
            "Queuing switch to client {} on {}",
            session.as_raw(),
            self.name
        );
        self.next_client = Some(target);
        self.disable_client(client)?;

        Ok(())
    }

    pub fn remove_client(
        &mut self,
        client: &rc::Rc<cell::RefCell<client::Client>>,
    ) -> anyhow::Result<()> {
        if self
            .next_client
            .as_ref()
            .is_some_and(|next_client| rc::Rc::ptr_eq(next_client, client))
        {
            self.next_client = None;
        }

        let (session, client_state) = {
            let client = client.borrow();
            (client.session_id, client.state)
        };

        self.clients
            .retain(|existing| !rc::Rc::ptr_eq(existing, client));
        client.borrow_mut().devices.clear();

        let was_current = self
            .active_client
            .as_ref()
            .is_some_and(|active_client| rc::Rc::ptr_eq(active_client, client));
        if was_current {
            self.active_client = None;
            let _ = self.activate();
        }

        if self.is_vt_bound() {
            if was_current && self.active_client.is_none() {
                log::debug!("Closing active VT");
                Self::vt_close(self.cur_vt)?;
            } else if let Some(session) = session
                && !was_current
                && client_state != SeatState::Closed
            {
                log::debug!("Closing inactive VT");
                Self::vt_close(session.as_raw())?;
            }
        }

        {
            let mut client = client.borrow_mut();
            client.state = SeatState::Closed;
            client.seat_name.clear();
        }

        log::info!(
            "Removed client {} from {}",
            session.map_or(0, session::SessionId::as_raw),
            self.name
        );

        Ok(())
    }
}
