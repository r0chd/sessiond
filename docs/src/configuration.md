# Configuration

`sessiond` reads TOML configuration from `/etc/sessiond.toml` by default.
Use `sessiond --config <path>` to load another file.

## Example

```toml
[users.alice]
linger = true
runtime-dir = "/run/user/{uid}"

[users.alice.env]
DBUS_SESSION_BUS_ADDRESS = "unix:path=/run/user/{uid}/bus"
```

User entries are keyed by login name. Supported fields are:

- `linger`: create user state before an active session exists.
- `runtime-dir`: runtime directory template; defaults to `/run/user/{uid}`.
- `env`: environment variables sent to newly created sessions.

The `{uid}` placeholder is available in `runtime-dir` and `env` values.
