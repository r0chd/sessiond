# Configuration

`sessiond` reads TOML configuration from `/etc/sessiond.toml` by default.
Use `sessiond --config <path>` to load a different file.

## Example

```toml
[users.alice]
linger = true
runtime-dir = "/run/user/{uid}"

[users.alice.env]
DBUS_SESSION_BUS_ADDRESS = "unix:path=/run/user/{uid}/bus"

[power]
reboot = ["/run/current-system/sw/bin/reboot"]
poweroff = ["/run/current-system/sw/bin/poweroff"]
suspend = ["/run/current-system/sw/bin/zzz", "-z"]
hibernate = ["/run/current-system/sw/bin/zzz", "-Z"]
```

## Top-level Fields

- `users`: per-user session defaults, keyed by user name.
- `power`: commands used by `sessionctl reboot`, `sessionctl poweroff`,
  `sessionctl suspend`, and `sessionctl hibernate`.

Unknown fields are rejected.

## Users

User entries are keyed by login name:

```toml
[users.alice]
linger = true
runtime-dir = "/run/user/{uid}"

[users.alice.env]
DBUS_SESSION_BUS_ADDRESS = "unix:path=/run/user/{uid}/bus"
```

Supported fields:

- `linger`: create the user state and runtime directory when `sessiond` starts,
  before the user has an active session. Defaults to `false`.
- `runtime-dir`: runtime directory template. Defaults to `/run/user/{uid}`.
- `env`: extra environment variables sent to newly created sessions.

Values in `runtime-dir` and `env` may use placeholder `{uid}`

## Power Commands

Power command fields are arrays where the first element is the program and the
remaining elements are arguments:

```toml
[power]
suspend = ["/run/current-system/sw/bin/zzz"]
hibernate = ["/run/current-system/sw/bin/zzz", "-H"]
reboot = ["/run/current-system/sw/bin/reboot" ]
poweroff = ["/run/current-system/sw/bin/poweroff" ]
```
