# Installation

## NixOS / finix

### Sources

With flakes:

```nix
{
  inputs.sessiond.url = "git+https://tangled.org/r0chd.pl/sessiond";
}
```

Use `inputs.sessiond` as `sessiond` in the examples below.

With npins:

```sh
npins add git --name sessiond https://tangled.org/r0chd.pl/sessiond --branch master
```

Import the pinned source in your configuration:

```nix
let
  sources = import ./npins;
  sessiond = import sources.sessiond.outPath { };
in
{
  # ...
}
```

### Configuration

Import the module and enable `sessiond`:

```nix
{
  imports = [
    sessiond.nixosModules.default # on NixOS
    sessiond.finixModules.default # on finix
  ];

  services.sessiond.enable = true;
}
```

The PAM module registers a session with `sessiond`.
It is enabled for the `login` service by default;
enable it for any other session-opening PAM service
too, such as `gdm`, `sddm`, `greetd` or `hilsed`

```nix
{
  security.pam.services.<service>.sessiond = true;
}
```

## Manual

### Build Dependencies

- Rust
- Cargo
- `pkg-config`
- PAM

### Build

```sh
cargo build --workspace --release
```

For polkit support, build with the `consolekit` feature:

```sh
cargo build --workspace --release --features consolekit
```

Build outputs can be found in `target/release`:

- `daemon`: the session daemon
- `ctl`: the control client
- `libpam.so`: the PAM module

## Other distributions

TODO
