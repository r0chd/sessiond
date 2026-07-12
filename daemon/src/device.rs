use crate::seat_v1::device;
use crate::{ecs, seat, seat_v1};
use bevy_ecs::{entity, world};
use rustix::{io, ioctl};
use std::ffi::c_int;
use std::os::fd;
use std::os::fd::AsFd;
use std::path;

impl<C: crate::cgroup::CgroupManager + 'static> hyprwire::Dispatch<device::Device>
    for crate::Sessiond<C>
{
    fn event(
        &mut self,
        object: &device::Device,
        event: <device::Device as hyprwire::Object>::Event<'_>,
    ) {
        let device::Event::Destroy = event;

        let Some(client_entity) =
            seat::client_entity_by_device_object(&self.world, &self.client_entities, object)
        else {
            return;
        };

        if let Err(err) = close_device(&mut self.world, client_entity, object) {
            log::error!("Could not close device: {err}");
        }
    }
}

pub fn close_device(
    world: &mut world::World,
    client_entity: entity::Entity,
    object: &seat_v1::device::Device,
) -> anyhow::Result<()> {
    let session_id = world
        .get::<ecs::Session>(client_entity)
        .map(|session| session.0)
        .ok_or_else(|| anyhow::anyhow!("client has no session id"))?;
    let seat_name = world
        .get::<ecs::SeatName>(client_entity)
        .map(|seat| seat.0.clone())
        .ok_or_else(|| anyhow::anyhow!("client has no seat"))?;
    let devices = world
        .get::<ecs::Devices>(client_entity)
        .ok_or_else(|| anyhow::anyhow!("client has no devices"))?;
    let idx = devices
        .0
        .iter()
        .position(|device_entity| contains_object(world, *device_entity, object))
        .ok_or_else(|| anyhow::anyhow!("device not found"))?;

    let device_entity = devices.0[idx];
    remove_object(world, device_entity, object);
    let device_path = device_path(world, device_entity)?;
    log::debug!(
        "Closing device {} for client {} on {}",
        device_path.display(),
        session_id.as_raw(),
        seat_name
    );

    if decrement_ref(world, device_entity) > 0 {
        return Ok(());
    }

    if let Err(err) = deactivate(world, device_entity) {
        log::error!(
            "Could not deactivate device {} for client {} on {}: {err}",
            device_path.display(),
            session_id.as_raw(),
            seat_name
        );
    }

    if let Some(mut devices) = world.get_mut::<ecs::Devices>(client_entity) {
        devices.0.remove(idx);
    }
    world.despawn(device_entity);

    Ok(())
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
        } else if path.to_str().is_some_and(|s| {
            s.starts_with("/dev/wskbd")
                || s.starts_with("/dev/wsmouse")
                || s.starts_with("/dev/wsmux")
        }) {
            Some(Self::Wscons)
        } else if path.to_str().is_some_and(|s| s.starts_with("/dev/hidraw")) {
            Some(Self::Hidraw)
        } else {
            None
        }
    }
}

pub(crate) fn matches_named_device(path: &path::Path, parent: &str, prefix: &str) -> bool {
    path.parent() == Some(path::Path::new(parent))
        && path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with(prefix))
}

pub fn activate(world: &mut world::World, entity: entity::Entity) -> anyhow::Result<()> {
    if world
        .get::<ecs::DeviceActive>(entity)
        .is_some_and(|active| active.0)
    {
        return Ok(());
    }

    let kind = device_kind(world, entity)?;
    match kind {
        DeviceKind::Drm => {
            let fd = world
                .get::<ecs::DeviceFd>(entity)
                .ok_or_else(|| anyhow::anyhow!("device has no fd"))?;
            drm_set_master(&fd.0)
                .map_err(|err| anyhow::anyhow!("Could not make device fd drm master: {err}"))?;
            world.entity_mut(entity).insert(ecs::DeviceActive(true));
        }
        DeviceKind::Evdev | DeviceKind::Hidraw => {
            return Err(anyhow::anyhow!("device type cannot be activated"));
        }
        DeviceKind::Wscons => {
            world.entity_mut(entity).insert(ecs::DeviceActive(true));
        }
    }

    Ok(())
}

pub fn deactivate(world: &mut world::World, entity: entity::Entity) -> anyhow::Result<()> {
    if !world
        .get::<ecs::DeviceActive>(entity)
        .is_some_and(|active| active.0)
    {
        return Ok(());
    }

    let kind = device_kind(world, entity)?;
    match kind {
        DeviceKind::Drm => {
            let fd = world
                .get::<ecs::DeviceFd>(entity)
                .ok_or_else(|| anyhow::anyhow!("device has no fd"))?;
            if let Err(err) = drm_drop_master(&fd.0) {
                log::error!("Could not revoke drm master on device fd: {err}");
                return Err(err.into());
            }
        }
        DeviceKind::Evdev => {
            let fd = world
                .get::<ecs::DeviceFd>(entity)
                .ok_or_else(|| anyhow::anyhow!("device has no fd"))?;
            if let Err(err) = evdev_revoke(&fd.0) {
                log::error!("Could not revoke evdev on device fd: {err}");
                return Err(err.into());
            }
        }
        DeviceKind::Hidraw => {
            let fd = world
                .get::<ecs::DeviceFd>(entity)
                .ok_or_else(|| anyhow::anyhow!("device has no fd"))?;
            if let Err(err) = hidraw_revoke(&fd.0) {
                log::error!("Could not revoke hidraw on device fd: {err}");
                return Err(err.into());
            }
        }
        DeviceKind::Wscons => {}
    }

    world.entity_mut(entity).insert(ecs::DeviceActive(false));
    Ok(())
}

pub fn matches_path(world: &world::World, entity: entity::Entity, path: &path::Path) -> bool {
    world
        .get::<ecs::DevicePath>(entity)
        .is_some_and(|device_path| device_path.0 == path)
}

pub fn add_object(world: &mut world::World, entity: entity::Entity, object: device::Device) {
    if let Some(mut ref_count) = world.get_mut::<ecs::DeviceRefCount>(entity) {
        ref_count.0 += 1;
    }
    if let Some(mut objects) = world.get_mut::<ecs::DeviceObjects>(entity) {
        objects.0.push(object);
    }
}

pub fn id(world: &world::World, entity: entity::Entity) -> Option<u32> {
    world.get::<ecs::DeviceId>(entity).map(|id| id.0)
}

pub fn device_kind(world: &world::World, entity: entity::Entity) -> anyhow::Result<DeviceKind> {
    world
        .get::<ecs::DeviceKind>(entity)
        .map(|kind| kind.0)
        .ok_or_else(|| anyhow::anyhow!("device has no kind"))
}

pub fn device_path(world: &world::World, entity: entity::Entity) -> anyhow::Result<path::PathBuf> {
    world
        .get::<ecs::DevicePath>(entity)
        .map(|path| path.0.clone())
        .ok_or_else(|| anyhow::anyhow!("device has no path"))
}

pub fn path_display(world: &world::World, entity: entity::Entity) -> String {
    device_path(world, entity).map_or_else(
        |_| "<unknown>".to_string(),
        |path| path.display().to_string(),
    )
}

pub fn absolute_path_string(
    world: &world::World,
    entity: entity::Entity,
) -> anyhow::Result<String> {
    let device_path = device_path(world, entity)?;
    Ok(path::absolute(&device_path)
        .unwrap_or(device_path)
        .display()
        .to_string())
}

pub fn last_object(world: &world::World, entity: entity::Entity) -> Option<&device::Device> {
    world
        .get::<ecs::DeviceObjects>(entity)
        .and_then(|objects| objects.0.last())
}

pub fn contains_object(
    world: &world::World,
    entity: entity::Entity,
    object: &device::Device,
) -> bool {
    world
        .get::<ecs::DeviceObjects>(entity)
        .is_some_and(|objects| objects.0.iter().any(|candidate| candidate == object))
}

pub fn remove_object(world: &mut world::World, entity: entity::Entity, object: &device::Device) {
    if let Some(mut objects) = world.get_mut::<ecs::DeviceObjects>(entity) {
        objects.0.retain(|candidate| candidate != object);
    }
}

pub fn send_revoked(world: &world::World, entity: entity::Entity) {
    if let Some(objects) = world.get::<ecs::DeviceObjects>(entity) {
        for object in &objects.0 {
            object.send_revoked();
        }
    }
}

pub fn decrement_ref(world: &mut world::World, entity: entity::Entity) -> usize {
    let Some(mut ref_count) = world.get_mut::<ecs::DeviceRefCount>(entity) else {
        return 0;
    };
    ref_count.0 = ref_count.0.saturating_sub(1);
    ref_count.0
}

pub fn send_opened(world: &world::World, entity: entity::Entity) -> anyhow::Result<()> {
    let Some(object) = last_object(world, entity) else {
        return Ok(());
    };
    let id = id(world, entity).ok_or_else(|| anyhow::anyhow!("device has no id"))?;
    let path = absolute_path_string(world, entity)?;
    let fd = world
        .get::<ecs::DeviceFd>(entity)
        .ok_or_else(|| anyhow::anyhow!("device has no fd"))?;
    object.send_opened(id, fd.0.as_fd(), path);
    Ok(())
}
