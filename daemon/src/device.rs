use crate::seat_v1::device;
use crate::session_core_v1;
use crate::session_core_v1::session_core_seat_device;
use crate::{cgroup, client, ecs, seat, seat_v1};
use bevy_ecs::{entity, world};
use rustix::{io, ioctl};
use std::ffi::c_int;
use std::os::fd;
use std::os::fd::AsFd;
use std::{fs, path};

const MAX_SEAT_DEVICES: u8 = 128;

impl<C: crate::cgroup::CgroupManager + 'static> hyprwire::Dispatch<device::Device>
    for crate::Sessiond<C>
{
    fn event(
        &mut self,
        object: &device::Device,
        event: <device::Device as hyprwire::Object>::Event<'_>,
    ) {
        let device::Event::Destroy = event;

        let Some(client_entity) = client::entity_by_device_object(&self.world, object) else {
            return;
        };

        if let Err(err) = close(&mut self.world, client_entity, object) {
            log::error!("Could not close device: {err}");
        }
    }
}

impl<C: cgroup::CgroupManager + 'static>
    hyprwire::Dispatch<session_core_seat_device::SessionCoreSeatDevice> for crate::Sessiond<C>
{
    fn event(
        &mut self,
        object: &session_core_seat_device::SessionCoreSeatDevice,
        event: <session_core_seat_device::SessionCoreSeatDevice as hyprwire::Object>::Event<'_>,
    ) {
        let session_core_seat_device::Event::Destroy = event;
        let Some(client_entity) = client::entity_by_core_device_object(&self.world, object) else {
            return;
        };
        let Some(device_entity) = entity_by_core_object(&self.world, client_entity, object) else {
            return;
        };
        remove_core_object(&mut self.world, device_entity, object);
    }
}

pub fn remove_core_object(
    world: &mut world::World,
    device_entity: entity::Entity,
    object: &session_core_seat_device::SessionCoreSeatDevice,
) {
    if let Some(mut objects) = world.get_mut::<ecs::SeatDeviceObjects>(device_entity) {
        objects.0.retain(|candidate| candidate.object != *object);
    }
}

pub fn notify_id_destroyed(world: &world::World, device_entity: entity::Entity, device_id: u32) {
    if let Some(objects) = world.get::<ecs::SeatDeviceObjects>(device_entity) {
        for object in &objects.0 {
            if object.id == device_id {
                object.object.send_destroyed();
            }
        }
    }
}

pub fn remove_id_objects(world: &mut world::World, device_entity: entity::Entity, device_id: u32) {
    if let Some(mut objects) = world.get_mut::<ecs::SeatDeviceObjects>(device_entity) {
        objects.0.retain(|candidate| candidate.id != device_id);
    }
}

fn close(
    world: &mut world::World,
    client_entity: entity::Entity,
    object: &seat_v1::device::Device,
) -> anyhow::Result<()> {
    let session_id = world
        .get::<ecs::SessionId>(client_entity)
        .map(|session| session.0)
        .ok_or_else(|| anyhow::anyhow!("client has no session id"))?;
    let name = world
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
    let device_id = world
        .get::<ecs::DeviceObjects>(device_entity)
        .and_then(|objects| {
            objects
                .0
                .iter()
                .find(|candidate| candidate.object == *object)
                .map(|candidate| candidate.id)
        })
        .ok_or_else(|| anyhow::anyhow!("device object not found"))?;
    remove_object(world, device_entity, object);
    let path = path(world, device_entity)?;
    log::debug!(
        "Closing device {} for client {} on {}",
        path.display(),
        session_id.as_raw(),
        name
    );

    client::notify_device_removed(world, client_entity, device_id);
    notify_id_destroyed(world, device_entity, device_id);
    remove_id_objects(world, device_entity, device_id);
    if decrement_ref(world, device_entity) > 0 {
        return Ok(());
    }

    if let Err(err) = deactivate(world, device_entity) {
        log::error!(
            "Could not deactivate device {} for client {} on {}: {err}",
            path.display(),
            session_id.as_raw(),
            name
        );
    }

    if let Some(mut devices) = world.get_mut::<ecs::Devices>(client_entity) {
        devices.0.remove(idx);
    }
    notify_destroyed(world, device_entity);
    world.despawn(device_entity);

    Ok(())
}

pub fn open(
    world: &mut world::World,
    seat_entity: entity::Entity,
    client_entity: entity::Entity,
    device: seat_v1::device::Device,
    path: &str,
) {
    if !client::is_active(world, seat_entity, client_entity) {
        device.send_failed(
            seat_v1::DeviceError::SeatDisabled,
            io::Errno::PERM.raw_os_error().cast_unsigned(),
            "open is only valid for the active seat client",
        );
        return;
    }

    let Ok(sanitized_path) = fs::canonicalize(path) else {
        device.send_failed(
            seat_v1::DeviceError::OpenFailed,
            io::Errno::NOENT.raw_os_error().cast_unsigned(),
            "could not canonicalize device path",
        );
        return;
    };

    if seat::state(world, client_entity).ok() != Some(seat::SeatState::Enabled) {
        device.send_failed(
            seat_v1::DeviceError::SeatDisabled,
            io::Errno::BUSY.raw_os_error().cast_unsigned(),
            "open is only valid while the seat is enabled",
        );
        return;
    }

    let Some(kind) = DeviceKind::from_path(&sanitized_path) else {
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

    let Some(device_entities) = world
        .get::<ecs::Devices>(client_entity)
        .map(|devices| devices.0.clone())
    else {
        device.send_failed(
            seat_v1::DeviceError::OpenFailed,
            io::Errno::INVAL.raw_os_error().cast_unsigned(),
            "client has no device storage",
        );
        return;
    };

    let open_handle_count = device_entities
        .iter()
        .map(|device_entity| object_count(world, *device_entity))
        .sum::<usize>();
    if open_handle_count >= usize::from(MAX_SEAT_DEVICES) {
        log::error!("Client exceeded max seat devices");
        device.send_failed(
            seat_v1::DeviceError::OpenFailed,
            io::Errno::MFILE.raw_os_error().cast_unsigned(),
            "client exceeded max seat devices",
        );
        return;
    }

    let device_id = device_entities
        .iter()
        .filter_map(|device_entity| max_id(world, *device_entity))
        .max()
        .map_or(1, |id| id + 1);

    if let Some(old_device) = device_entities
        .iter()
        .find(|device_entity| matches_path(world, **device_entity, &sanitized_path))
        .copied()
    {
        add_object(world, old_device, device, device_id);
        client::notify_device_added(world, client_entity, device_id);
        if let Err(err) = send_opened(world, old_device) {
            log::error!("Could not send opened device event: {err}");
        }
        return;
    }

    let session_id = client::session_id(world, client_entity)
        .map(crate::session::SessionId::as_raw)
        .unwrap_or_default();
    let name = seat::name(world, seat_entity).unwrap_or_else(|_| "<unknown>".to_string());

    log::debug!(
        "Opening device {} for client {} on {}",
        sanitized_path.display(),
        session_id,
        name
    );

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
        Err(err) => {
            log::error!("Could not open file: {err}");
            device.send_failed(
                seat_v1::DeviceError::OpenFailed,
                err.raw_os_error().cast_unsigned(),
                "could not open device",
            );
            return;
        }
    };

    let device_entity = ecs::insert_device(
        world,
        device,
        sanitized_path,
        kind,
        device_id,
        matches!(kind, DeviceKind::Evdev | DeviceKind::Hidraw),
        fd,
    );

    if let Err(err) = activate(world, device_entity) {
        let path = path_display(world, device_entity);
        log::error!("Could not activate {path}: {err}");
        let errno = err
            .root_cause()
            .downcast_ref::<io::Errno>()
            .copied()
            .unwrap_or(io::Errno::INVAL);
        if let Some(object) = last_object(world, device_entity) {
            object.send_failed(
                seat_v1::DeviceError::OpenFailed,
                errno.raw_os_error().cast_unsigned(),
                "could not activate device",
            );
        }
        world.despawn(device_entity);
        return;
    }

    if let Err(err) = send_opened(world, device_entity) {
        log::error!("Could not send opened device event: {err}");
    }
    if let Some(mut devices) = world.get_mut::<ecs::Devices>(client_entity) {
        devices.0.push(device_entity);
    }
    client::notify_device_added(world, client_entity, device_id);
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

pub fn matches_named_device(path: &path::Path, parent: &str, prefix: &str) -> bool {
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

    let kind = kind(world, entity)?;
    match kind {
        DeviceKind::Drm => {
            let fd = world
                .get::<ecs::DeviceFd>(entity)
                .ok_or_else(|| anyhow::anyhow!("device has no fd"))?;
            drm_set_master(&fd.0)
                .map_err(|err| anyhow::anyhow!("Could not make device fd drm master: {err}"))?;
            world.entity_mut(entity).insert(ecs::DeviceActive(true));
            notify_active(world, entity, true);
        }
        DeviceKind::Evdev | DeviceKind::Hidraw => {
            return Err(anyhow::anyhow!("device type cannot be activated"));
        }
        DeviceKind::Wscons => {
            world.entity_mut(entity).insert(ecs::DeviceActive(true));
            notify_active(world, entity, true);
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

    let kind = kind(world, entity)?;
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
    notify_active(world, entity, false);
    Ok(())
}

fn notify_active(world: &world::World, entity: entity::Entity, active: bool) {
    if let Some(objects) = world.get::<ecs::SeatDeviceObjects>(entity) {
        for object in &objects.0 {
            object.object.send_active(u32::from(active));
        }
    }
}

pub fn matches_path(world: &world::World, entity: entity::Entity, path: &path::Path) -> bool {
    world
        .get::<ecs::DevicePath>(entity)
        .is_some_and(|device_path| device_path.0 == path)
}

pub fn add_object(
    world: &mut world::World,
    entity: entity::Entity,
    object: device::Device,
    id: u32,
) {
    if let Some(mut ref_count) = world.get_mut::<ecs::DeviceRefCount>(entity) {
        ref_count.0 += 1;
    }
    if let Some(mut objects) = world.get_mut::<ecs::DeviceObjects>(entity) {
        objects.0.push(ecs::DeviceObject { object, id });
    }
}

pub fn max_id(world: &world::World, entity: entity::Entity) -> Option<u32> {
    world
        .get::<ecs::DeviceObjects>(entity)
        .and_then(|objects| objects.0.iter().map(|handle| handle.id).max())
}

pub fn object_count(world: &world::World, entity: entity::Entity) -> usize {
    world
        .get::<ecs::DeviceObjects>(entity)
        .map_or(0, |objects| objects.0.len())
}

pub fn kind(world: &world::World, entity: entity::Entity) -> anyhow::Result<DeviceKind> {
    world
        .get::<ecs::DeviceKind>(entity)
        .map(|kind| kind.0)
        .ok_or_else(|| anyhow::anyhow!("device has no kind"))
}

pub fn path(world: &world::World, entity: entity::Entity) -> anyhow::Result<path::PathBuf> {
    world
        .get::<ecs::DevicePath>(entity)
        .map(|path| path.0.clone())
        .ok_or_else(|| anyhow::anyhow!("device has no path"))
}

pub fn path_display(world: &world::World, entity: entity::Entity) -> String {
    path(world, entity).map_or_else(
        |_| "<unknown>".to_string(),
        |path| path.display().to_string(),
    )
}

pub fn absolute_path_string(
    world: &world::World,
    entity: entity::Entity,
) -> anyhow::Result<String> {
    let path = path(world, entity)?;
    Ok(path::absolute(&path).unwrap_or(path).display().to_string())
}

pub fn last_object(world: &world::World, entity: entity::Entity) -> Option<&device::Device> {
    world
        .get::<ecs::DeviceObjects>(entity)
        .and_then(|objects| objects.0.last())
        .map(|handle| &handle.object)
}

pub fn contains_object(
    world: &world::World,
    entity: entity::Entity,
    object: &device::Device,
) -> bool {
    world
        .get::<ecs::DeviceObjects>(entity)
        .is_some_and(|objects| {
            objects
                .0
                .iter()
                .any(|candidate| candidate.object == *object)
        })
}

pub fn remove_object(world: &mut world::World, entity: entity::Entity, object: &device::Device) {
    if let Some(mut objects) = world.get_mut::<ecs::DeviceObjects>(entity) {
        objects.0.retain(|candidate| candidate.object != *object);
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
    let Some(handle) = world
        .get::<ecs::DeviceObjects>(entity)
        .and_then(|objects| objects.0.last())
    else {
        return Ok(());
    };
    let path = absolute_path_string(world, entity)?;
    let fd = world
        .get::<ecs::DeviceFd>(entity)
        .ok_or_else(|| anyhow::anyhow!("device has no fd"))?;
    handle.object.send_opened(handle.id, fd.0.as_fd(), path);
    Ok(())
}

pub fn entity_by_id(
    world: &world::World,
    client_entity: entity::Entity,
    device_id: u32,
) -> Option<entity::Entity> {
    world
        .get::<ecs::Devices>(client_entity)?
        .0
        .iter()
        .copied()
        .find(|device_entity| {
            world
                .get::<ecs::DeviceObjects>(*device_entity)
                .is_some_and(|objects| objects.0.iter().any(|object| object.id == device_id))
        })
}

pub fn entity_by_core_object(
    world: &world::World,
    client_entity: entity::Entity,
    object: &session_core_seat_device::SessionCoreSeatDevice,
) -> Option<entity::Entity> {
    world
        .get::<ecs::Devices>(client_entity)?
        .0
        .iter()
        .copied()
        .find(|device_entity| {
            world
                .get::<ecs::SeatDeviceObjects>(*device_entity)
                .is_some_and(|objects| {
                    objects
                        .0
                        .iter()
                        .any(|candidate| candidate.object == *object)
                })
        })
}

pub const fn kind_event(kind: DeviceKind) -> session_core_v1::SeatDeviceKind {
    match kind {
        DeviceKind::Drm => session_core_v1::SeatDeviceKind::Drm,
        DeviceKind::Evdev => session_core_v1::SeatDeviceKind::Evdev,
        DeviceKind::Hidraw => session_core_v1::SeatDeviceKind::Hidraw,
        DeviceKind::Wscons => session_core_v1::SeatDeviceKind::Wscons,
    }
}

pub fn add_core_object(
    world: &mut world::World,
    device_entity: entity::Entity,
    device_id: u32,
    object: session_core_seat_device::SessionCoreSeatDevice,
) {
    object.send_device_id(device_id);
    if let Ok(path) = absolute_path_string(world, device_entity) {
        object.send_path(path);
    }
    if let Some(kind) = world.get::<ecs::DeviceKind>(device_entity) {
        object.send_kind(kind_event(kind.0));
    }
    if let Some(active) = world.get::<ecs::DeviceActive>(device_entity) {
        object.send_active(u32::from(active.0));
    }
    object.send_done();

    if let Some(mut objects) = world.get_mut::<ecs::SeatDeviceObjects>(device_entity) {
        objects.0.push(ecs::SeatDeviceObject {
            object,
            id: device_id,
        });
    }
}

pub fn notify_removed(
    world: &world::World,
    client_entity: entity::Entity,
    device_entity: entity::Entity,
) {
    if let Some(objects) = world.get::<ecs::DeviceObjects>(device_entity) {
        for device_object in &objects.0 {
            client::notify_device_removed(world, client_entity, device_object.id);
        }
    }
    notify_destroyed(world, device_entity);
}

pub fn notify_destroyed(world: &world::World, device_entity: entity::Entity) {
    if let Some(objects) = world.get::<ecs::SeatDeviceObjects>(device_entity) {
        for object in &objects.0 {
            object.object.send_destroyed();
        }
    }
}
