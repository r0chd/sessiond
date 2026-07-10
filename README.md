# sessiond

[![coverage](https://coverage-badge.web.garage.r0chd.pl/coverage.svg)](https://tangled.org/parzivale.tngl.sh/sessiond/pipelines)

`sessiond` is a session management daemon for systems that do not want to
depend on `(e)logind`.

Session information is exposed over the Hyprwire protocol, letting clients
determine which session belongs to which user and inspect session state.
The protocol is split across three specifications:

- [protocols/session_management_v1.xml](protocols/session_management_v1.xml): creating and destroying sessions
- [protocols/session_core_v1.xml](protocols/session_core_v1.xml): querying session information
- [protocols/seat_v1.xml](protocols/seat_v1.xml): acquiring seats

`sessiond` implements parts of the `org.freedesktop.ConsoleKit` D-Bus
interface for drop in polkit support.

Seat tracking is currently a work in progress; full multi-seat support is
planned.

## Features

- Session registration and tracking via a PAM module
- Session state (active/inactive, local/remote, owning uid) exposed over Hyprwire
- Partial ConsoleKit D-Bus interface for polkit
- Seat and device tracking (work in progress)

## Installation

See [docs/INSTALL.md](docs/INSTALL.md).

## Known Issues

See [docs/KNOWN_ISSUES.md](docs/KNOWN_ISSUES.md).
