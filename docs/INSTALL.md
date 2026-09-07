# Installation

## NixOS / finix

With flakes, add `sessiond` as an input:

```nix
{
  inputs.sessiond.url = "git+https://tangled.org/r0chd.pl/sessiond";
}
```

Then import the module and enable `sessiond`:

```nix
{ inputs, ... }:
{
  imports = [
    inputs.sessiond.nixosModules.default # on NixOS
    inputs.sessiond.finixModules.default # on finix
  ];

  services.sessiond.enable = true;
}
```

With npins, pin the source:

```sh
npins add git --name sessiond https://tangled.org/r0chd.pl/sessiond --branch master
```

Then import the pinned module and enable `sessiond`:

```nix
{ ... }:
let
  sources = import ./npins;
  sessiond = import sources.sessiond.outPath { };
in
{
  imports = [
    sessiond.nixosModules.default # on NixOS
    sessiond.finixModules.default # on finix
  ];

  services.sessiond.enable = true;
}
```

See [Configuration](CONFIGURATION.md) for `services.sessiond.settings` and the
equivalent `sessiond.toml` format.

The PAM module registers a session with `sessiond`.
It is enabled for the `login` service by default;
enable it for any other session-opening PAM service
too, such as `gdm`, `sddm`, `greetd` or `hilsed`.

On NixOS:

```nix
{
  security.pam.services.<service>.rules.session.sessiond = {
    control = "optional";
    modulePath = "${config.services.sessiond.package}/lib/security/pam_sessiond.so";
    order = 900;
  };
}
```

On finix:

```nix
{ lib, config, ... }:
{
  security.pam.services.<service>.text = lib.mkAfter ''
    session   optional   ${config.services.sessiond.package}/lib/security/pam_sessiond.so
  '';
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

- `sessiond`: the session daemon
- `sessionctl`: the control client
- `libpam.so`: the PAM module

## Other distributions

TODO
