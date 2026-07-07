# sessiond

`sessiond` is a Linux session and seat management daemon.

## Build Dependencies

- Rust
- Cargo
- `pkg-config`
- PAM

## Build

```sh
cargo build --workspace --release
```

Build outputs can be found in `target/release`:

- `daemon`: the session daemon
- `ctl`: the control client
- `libpam.so`: the PAM module

## Installation

See [docs/INSTALL.md](docs/INSTALL.md).
