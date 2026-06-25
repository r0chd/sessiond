use crate::seat_v1::device;
use rustix::{io, ioctl};
use std::ffi::c_int;
use std::os::fd;
use std::path;

impl hyprwire::Dispatch<device::Device> for crate::Sessiond {
    fn event(
        &mut self,
        object: &device::Device,
        event: <device::Device as hyprwire::Object>::Event<'_>,
    ) {
        let device::Event::Destroy = event;

        let Some(client) = self.clients.iter().find(|client| {
            client
                .borrow()
                .devices
                .iter()
                .any(|device| device.contains_object(object))
        }) else {
            return;
        };

        if let Err(err) = client.borrow_mut().close_device(object) {
            log::error!("Could not close device: {err}");
        }
    }
}

const DRM_SET_MASTER: ioctl::Opcode = ioctl::opcode::none(b'd', 0x1e);
const DRM_DROP_MASTER: ioctl::Opcode = ioctl::opcode::none(b'd', 0x1f);
const EVIOCREVOKE: ioctl::Opcode = ioctl::opcode::write::<c_int>(b'E', 0x91);
const HIDIOCREVOKE: ioctl::Opcode = ioctl::opcode::write::<c_int>(b'H', 0x0d);

fn drm_set_master<Fd: fd::AsFd>(fd: Fd) -> io::Result<()> {
    unsafe { ioctl::ioctl(fd, ioctl::NoArg::<DRM_SET_MASTER>::new()) }
}

fn drm_drop_master<Fd: fd::AsFd>(fd: Fd) -> io::Result<()> {
    unsafe { ioctl::ioctl(fd, ioctl::NoArg::<DRM_DROP_MASTER>::new()) }
}

fn evdev_revoke<Fd: fd::AsFd>(fd: Fd) -> io::Result<()> {
    unsafe { ioctl::ioctl(fd, ioctl::IntegerSetter::<EVIOCREVOKE>::new_usize(0)) }
}

fn hidraw_revoke<Fd: fd::AsFd>(fd: Fd) -> io::Result<()> {
    unsafe { ioctl::ioctl(fd, ioctl::IntegerSetter::<HIDIOCREVOKE>::new_usize(0)) }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceKind {
    Evdev,
    Drm,
    Wscons,
    Hidraw,
}

impl DeviceKind {
    pub fn from_path(path: &path::Path) -> Option<Self> {
        if matches_named_device(path, "/dev/input", "event") {
            Some(Self::Evdev)
        } else if path.starts_with("/dev/dri/") || path.starts_with("/dev/drm/") {
            Some(Self::Drm)
        } else if path.starts_with("/dev/wskbd")
            || path.starts_with("/dev/wsmouse")
            || path.starts_with("/dev/wsmux")
        {
            Some(Self::Wscons)
        } else if path.starts_with("/dev/hidraw") {
            Some(Self::Hidraw)
        } else {
            None
        }
    }
}

fn matches_named_device(path: &path::Path, parent: &str, prefix: &str) -> bool {
    path.parent() == Some(path::Path::new(parent))
        && path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with(prefix))
}

pub struct Device {
    objects: Vec<device::Device>,
    path: path::PathBuf,
    ref_cnt: usize,
    r#type: DeviceKind,
    id: u32,
    active: bool,
    fd: fd::OwnedFd,
}

impl Device {
    pub(crate) fn new(
        object: device::Device,
        path: path::PathBuf,
        r#type: DeviceKind,
        id: u32,
        active: bool,
        fd: fd::OwnedFd,
    ) -> Self {
        Self {
            objects: vec![object],
            path,
            ref_cnt: 1,
            r#type,
            id,
            active,
            fd,
        }
    }

    pub(crate) fn activate(&mut self) -> anyhow::Result<()> {
        if self.active {
            return Ok(());
        }

        match self.r#type {
            DeviceKind::Drm => {
                drm_set_master(&self.fd)
                    .map_err(|err| anyhow::anyhow!("Could not make device fd drm master: {err}"))?;
                self.active = true;
            }
            DeviceKind::Evdev | DeviceKind::Hidraw => {
                return Err(anyhow::anyhow!("device type cannot be activated"));
            }
            DeviceKind::Wscons => {
                self.active = true;
            }
        }

        Ok(())
    }

    pub(crate) fn deactivate(&mut self) -> anyhow::Result<()> {
        if !self.active {
            return Ok(());
        }

        match self.r#type {
            DeviceKind::Drm => {
                if let Err(err) = drm_drop_master(&self.fd) {
                    log::error!("Could not revoke drm master on device fd: {err}");
                    return Err(err.into());
                }
            }
            DeviceKind::Evdev => {
                if let Err(err) = evdev_revoke(&self.fd) {
                    log::error!("Could not revoke evdev on device fd: {err}");
                    return Err(err.into());
                }
            }
            DeviceKind::Hidraw => {
                if let Err(err) = hidraw_revoke(&self.fd) {
                    log::error!("Could not revoke hidraw on device fd: {err}");
                    return Err(err.into());
                }
            }
            DeviceKind::Wscons => {}
        }

        self.active = false;
        Ok(())
    }

    pub(crate) fn matches_path(&self, path: &path::Path) -> bool {
        self.path == path
    }

    pub(crate) fn add_object(&mut self, object: device::Device) {
        self.ref_cnt += 1;
        self.objects.push(object);
    }

    pub(crate) const fn id(&self) -> u32 {
        self.id
    }

    pub(crate) const fn kind(&self) -> DeviceKind {
        self.r#type
    }

    pub(crate) const fn fd(&self) -> &fd::OwnedFd {
        &self.fd
    }

    pub(crate) fn path(&self) -> &path::Path {
        &self.path
    }

    pub(crate) fn absolute_path_string(&self) -> String {
        path::absolute(&self.path)
            .unwrap_or_else(|_| self.path.clone())
            .display()
            .to_string()
    }

    pub(crate) fn last_object(&self) -> Option<&device::Device> {
        self.objects.last()
    }

    pub(crate) fn contains_object(&self, object: &device::Device) -> bool {
        self.objects.iter().any(|candidate| candidate == object)
    }

    pub(crate) fn remove_object(&mut self, object: &device::Device) {
        self.objects.retain(|candidate| candidate != object);
    }

    pub(crate) fn send_revoked(&self) {
        for object in &self.objects {
            object.send_revoked();
        }
    }

    pub(crate) const fn decrement_ref(&mut self) -> usize {
        self.ref_cnt = self.ref_cnt.saturating_sub(1);
        self.ref_cnt
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        if !self.active {
            return;
        }

        if let Err(err) = self.deactivate() {
            log::error!(
                "Could not deactivate {} during drop: {err}",
                self.path.display()
            );
        }
        self.active = false;
    }
}

#[cfg(test)]
mod tests {
    use super::{DeviceKind, matches_named_device};
    use std::path::Path;

    #[test]
    fn recognizes_evdev_event_nodes() {
        assert_eq!(
            DeviceKind::from_path(Path::new("/dev/input/event0")),
            Some(DeviceKind::Evdev)
        );
        assert_eq!(
            DeviceKind::from_path(Path::new("/dev/input/event12")),
            Some(DeviceKind::Evdev)
        );
    }

    #[test]
    fn does_not_match_partial_parent_paths() {
        assert!(!matches_named_device(
            Path::new("/dev/input/by-path/event0"),
            "/dev/input",
            "event"
        ));
        assert_eq!(DeviceKind::from_path(Path::new("/dev/input")), None);
    }
}
