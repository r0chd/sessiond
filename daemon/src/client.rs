use crate::seat_v1;
use crate::seat_v1::seat;
use crate::{device, session};
use anyhow::Context;

pub struct Client {
    pub(crate) seat_name: String,
    pub(crate) object: seat::Seat,
    pub(crate) devices: Vec<device::Device>,
    pub(crate) state: crate::seat::SeatState,
    pub(crate) session_id: Option<session::SessionId>,
}

impl PartialEq for Client {
    fn eq(&self, other: &Self) -> bool {
        self.object == other.object
    }
}

impl Eq for Client {}

impl Client {
    pub(crate) fn session_id(&self) -> anyhow::Result<session::SessionId> {
        self.session_id.context("client has no session id")
    }

    pub(crate) fn close_device(&mut self, object: &seat_v1::device::Device) -> anyhow::Result<()> {
        let session_id = self.session_id()?;
        let idx = self
            .devices
            .iter()
            .position(|device| device.contains_object(object))
            .ok_or_else(|| anyhow::anyhow!("device not found"))?;

        let device = &mut self.devices[idx];
        device.remove_object(object);
        log::debug!(
            "Closing device {} for client {} on {}",
            device.path().display(),
            session_id.as_raw(),
            self.seat_name
        );

        if device.decrement_ref() > 0 {
            return Ok(());
        }

        if let Err(err) = device.deactivate() {
            log::error!(
                "Could not deactivate device {} for client {} on {}: {err}",
                device.path().display(),
                session_id.as_raw(),
                self.seat_name
            );
        }

        self.devices.remove(idx);

        Ok(())
    }
}
