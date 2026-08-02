# sessiond

![coverage](https://coverage-badge.web.garage.r0chd.pl/coverage.svg)

`sessiond` is a session management daemon for systems that do not want to
depend on `(e)logind`.

It exposes session management and VT activity over the Hyprwire protocol:

- [protocols/session_management_v1.xml](protocols/session_management_v1.xml): unsafe session creation and destruction meant to be used with pam
- [protocols/session_management_v2.xml](protocols/session_management_v2.xml): owned session creation and destruction
- [protocols/session_core_v1.xml](protocols/session_core_v1.xml): querying session information

## What is it?

`sessiond` provides a session manager for registering, tracking, and querying
user sessions over a public Hyprwire API. It watches virtual terminals and
updates session foreground activity, and provides a partial
`org.freedesktop.ConsoleKit` D-Bus interface for polkit integration.

## What is it not?

`sessiond` is not responsible for starting user service managers or managing
seats. Starting and integrating a user service manager, as well as acquiring
seats and managing seat devices, should be handled by a `sessiond` client.

See [Clients](docs/CLIENTS.md) for known `sessiond` clients.

## Documentation

- [Installation](docs/INSTALL.md)
- [Configuration](docs/CONFIGURATION.md)
- [Clients](docs/CLIENTS.md)
- [Known issues](docs/KNOWN_ISSUES.md).
