# sessiond

A modular session management daemon for Linux.

sessiond registers, tracks and exposes login sessions.
The core stays small: device access, power actions,
backlight control and per-user hooks are handled by
optional client daemons.

## Features

- Session registration through a PAM module (`pam_sessiond.so`)
- Session and user tracking, with a cgroup for each session
- Lingering users
- Inhibitors
- Session state recovery across daemon restarts
- Partial ConsoleKit D-Bus interface for drop in replacement
- sd_notify readiness notifications

## Components

| Component | Description |
|---|---|
| `sessiond` | The core daemon |
| `sessionctl` | Command-line tool to query and control sessions |
| `pam_sessiond.so` | PAM module that registers sessions |
| `sessiond-uaccess` | Gives active local sessions access to devices (POSIX ACLs) |
| `sessiond-power` | Handles authorized power actions |
| `sessiond-backlight` | Controls backlight and LED brightness for active local sessions |
| `sessiond-hooks` | Launches and supervises per-user session hooks |
| `libsession` | Generic library for querying session info across all session managers |

## Installation

NixOS and finix modules are provided:

```nix
services.sessiond.enable = true;
```

See the [documentation](https://r0chd.tngl.sh/sessiond) for NixOS, finix
and building from source.

## Documentation

https://r0chd.tngl.sh/sessiond

## Development

Development happens on [Tangled](https://tangled.org/r0chd.pl/sessiond).
The GitHub repository is a read-only mirror.

Contributions created with AI tools are not accepted. See
[CONTRIBUTING.md](CONTRIBUTING.md) for details.

## License

GPL-3.0-only. See [LICENSE](LICENSE).
