# Configuration

`sessiond` reads TOML configuration from `/etc/sessiond.toml` by default.
Use `sessiond --config <path>` to load a different file.

## Example

```toml
[management]
user = "root"
group = "root"

[users.alice]
linger = true
runtime-dir = "/run/user/{uid}"

[users.alice.env]
DBUS_SESSION_BUS_ADDRESS = "unix:path={runtime-dir}/bus"

[power]
reboot = ["/run/current-system/sw/bin/reboot"]
poweroff = ["/run/current-system/sw/bin/poweroff"]
suspend = ["/run/current-system/sw/bin/zzz", "-z"]
hibernate = ["/run/current-system/sw/bin/zzz", "-Z"]
```

## Top-level Fields

- `management`: access rules for session management operations. Defaults to
  `root:root`.
- `users`: per-user session defaults, keyed by user name.
- `power`: commands used by `sessionctl reboot`, `sessionctl poweroff`,
  `sessionctl suspend`, and `sessionctl hibernate`.

Unknown fields are rejected.

## Management Access

`management.user` and `management.group` grant privileged session management
access. Management access can modify users and can run power commands
even when other users have active sessions.

Both fields default to `root`.

## Users

User entries are keyed by login name:

```toml
[users.alice]
linger = true
runtime-dir = "/run/user/{uid}"

[users.alice.env]
DBUS_SESSION_BUS_ADDRESS = "unix:path={runtime-dir}/bus"
```

Supported fields:

- `linger`: create the user state and runtime directory when `sessiond` starts,
  before the user has an active session. Defaults to `false`.
- `runtime-dir`: runtime directory template. Defaults to `/run/user/{uid}`.
- `env`: extra environment variables sent to newly created sessions.

Values in `runtime-dir` and `env` may use these placeholders:

- `{uid}`: numeric user ID.
- `{runtime-dir}`: the resolved runtime directory. This placeholder is only
  expanded in `env` values.

## Power Commands

Power command fields are arrays where the first element is the program and the
remaining elements are arguments:

```toml
[power]
suspend = ["/run/current-system/sw/bin/loginctl", "suspend"]
```
