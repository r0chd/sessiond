use crate::device::{matches_named_device, DeviceKind};
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
fn recognizes_drm_devices() {
    assert_eq!(
        DeviceKind::from_path(Path::new("/dev/dri/card0")),
        Some(DeviceKind::Drm)
    );
    assert_eq!(
        DeviceKind::from_path(Path::new("/dev/dri/renderD128")),
        Some(DeviceKind::Drm)
    );
    assert_eq!(
        DeviceKind::from_path(Path::new("/dev/drm/card0")),
        Some(DeviceKind::Drm)
    );
}

#[test]
fn recognizes_wscons_devices() {
    assert_eq!(
        DeviceKind::from_path(Path::new("/dev/wskbd0")),
        Some(DeviceKind::Wscons)
    );
    assert_eq!(
        DeviceKind::from_path(Path::new("/dev/wsmouse0")),
        Some(DeviceKind::Wscons)
    );
    assert_eq!(
        DeviceKind::from_path(Path::new("/dev/wsmux0")),
        Some(DeviceKind::Wscons)
    );
}

#[test]
fn recognizes_hidraw_devices() {
    assert_eq!(
        DeviceKind::from_path(Path::new("/dev/hidraw0")),
        Some(DeviceKind::Hidraw)
    );
    assert_eq!(
        DeviceKind::from_path(Path::new("/dev/hidraw9")),
        Some(DeviceKind::Hidraw)
    );
}

#[test]
fn unknown_paths_return_none() {
    assert_eq!(DeviceKind::from_path(Path::new("/dev/sda")), None);
    assert_eq!(DeviceKind::from_path(Path::new("/dev/null")), None);
    assert_eq!(DeviceKind::from_path(Path::new("/dev/video0")), None);
    assert_eq!(DeviceKind::from_path(Path::new("/dev/tty1")), None);
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
