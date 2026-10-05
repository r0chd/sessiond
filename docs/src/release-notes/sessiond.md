# sessiond changelog

## v0.1.0 - 2026-07-26

Initial public release of `sessiond`.

## v0.1.1 - 2026-07-28

**Added**
- Added a DBus system policy for the ConsoleKit interface

## v0.2.0 - 2026-08-08

**Changed**
- Changed the default sessiond socket path to `/run/sessiond/sessiond.sock`

**Removed**
- Removed seat management in favor of separate seat client

**Added**
- sd_notify lifecycle notifications (STOPPING, READY)
- Added the `session_power_v1` protocol for system power control
- Added session state recovery across daemon restarts

## v0.2.1 - 2026-08-31

**Fixed**
- Derive the cgroup root from `/proc/self/cgroup`

**Added**
- inhibit protocol

## v0.2.2 - 2026-09-06

**Fixed**
- Fall back to `XDG_VTNR` when `PAM_TTY` is unset

## v0.2.3 - 2026-09-07

**Fixed**
- `XDG_VTNR` env parsing in pam module

## v0.3.0 - 2026-10-06

**Added**
- `get_session` Hyprwire method for the current process; use `get_session_by_id` for ID lookups
- Save unrecoverable sessions to the state file
- concept of display session (oldest local user session)

**Changed**
- Require an explicit leader PID when creating a session, default to Unix credentials when no leader is sent
- Stop exporting `XDG_SESSION_ID` when the ConsoleKit feature is enabled

**Removed**
- power management in favor of power client
- Configuration of session environment variables
