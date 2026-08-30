# Changelog

## v0.1.0 - 2026-07-26

Initial public release of `sessiond`.

## v0.1.1 - 2026-07-28

### Added
- Added a DBus system policy for the ConsoleKit interface

## v0.2.0 - 2026-08-08

### Changed
- Changed the default sessiond socket path to `/run/sessiond/sessiond.sock`

### Removed
- Removed seat management in favor of separate seat client

### Added
- sd_notify lifecycle notifications (STOPPING, READY)
- Added the `session_power_v1` protocol for system power control
- Added session state recovery across daemon restarts

## v0.2.1 - 2026-08-31

### Fixed
- Derive the cgroup root from `/proc/self/cgroup`

### Added
- inhibit protocol
