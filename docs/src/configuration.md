# Configuration

`sessiond` reads TOML configuration from `/etc/sessiond.toml` by default.
Use `sessiond --config <path>` to load another file.

## Example

```toml
[users.alice]
linger = true
runtime-dir = "/run/user/{uid}"
```

User entries are keyed by login name. Supported fields are:

- `linger`: create user state before an active session exists.
- `runtime-dir`: runtime directory template; defaults to `/run/user/{uid}`.
