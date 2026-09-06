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
```

## Top-level Fields

- `users`: per-user session defaults, keyed by user name.

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
