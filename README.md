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

Import the NixOS module:

```nix
{
  imports = [
    sessiond.nixosModules.default
  ];
}
```

or if you're using finix:

```nix
{
  imports = [
    sessiond.finixModules.default
  ];
}
```

## Configuration

Enable `sessiond`:

```nix
{
  services.sessiond.enable = true;
}
```

To enable the PAM module for another PAM service:

```nix
{
  security.pam.services.<service>.sessiond = true;
}
```

Available options:

```nix
{
  services.sessiond = {
    enable = true;
    package = pkgs.sessiond;
    seat = {
      user = "root";
      group = "seat";
    };
  };
}
```
