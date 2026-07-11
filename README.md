# sessiond

[![coverage](https://coverage-badge.web.garage.r0chd.pl/coverage.svg)](https://tangled.org/parzivale.tngl.sh/sessiond/pipelines)

`sessiond` is a session management daemon for systems that do not want to
depend on `(e)logind`.

It exposes session and seat management over the Hyprwire protocol:

- [protocols/session_management_v1.xml](protocols/session_management_v1.xml): creating and destroying sessions
- [protocols/session_core_v1.xml](protocols/session_core_v1.xml): querying session information
- [protocols/seat_v1.xml](protocols/seat_v1.xml): acquiring seats

## What is it?

`sessiond` provides:

- A session manager for registering, tracking, and querying user sessions
- Public API over hyprwire protocol
- A seat manager for acquiring seats and managing seat devices
- A partial `org.freedesktop.ConsoleKit` D-Bus interface for polkit integration

## Documentation

- [Installation](docs/INSTALL.md)
- [Seat manager](docs/SEAT_MANAGER.md)
- [Known issues](docs/KNOWN_ISSUES.md).
