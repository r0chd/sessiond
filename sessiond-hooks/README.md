# sessiond-hooks

`sessiond-hooks` is a `sessiond` client that launches and supervises a per-user
session hook (`dinit --user`, `runsvdir`, …) started for every unique UID.

## Installation

`sessiond-hooks` requires `sessiond` to be installed and enabled first.

On finix with flakes:

```nix
{
  inputs.sessiond-hooks.url = "git+https://tangled.org/r0chd.pl/sessiond-hooks";
}
```

```nix
{ inputs, lib, pkgs, ... }:
{
  imports = [
    inputs.sessiond-hooks.finixModules.default
  ];

  services.sessiond-hooks = {
    package = inputs.sessiond-hooks.packages.${pkgs.system}.default;
    hooks.dinit.settings = {
      on_start = {
        exec = lib.getExe' pkgs.dinit "dinit";
        args = [ "--user" "--services-dir" "~/.config/dinit.d" ];
      };
      restart = {
        on = "Failure";
        delay = 5;
      };
    };
  };
}
```

With npins:

```sh
npins add git --name sessiond-hooks https://tangled.org/r0chd.pl/sessiond-hooks --branch master
```

```nix
{ lib, pkgs, ... }:
let
  sources = import ./npins;
  sessiond-hooks = import sources.sessiond-hooks.outPath { };
in
{
  imports = [
    sessiond-hooks.finixModules.default
  ];

  services.sessiond-hooks = {
    package = sessiond-hooks.packages.sessiond-hooks;
    hooks.dinit.settings = {
      on_start = {
        exec = lib.getExe' pkgs.dinit "dinit";
        args = [ "--user" "--services-dir" "~/.config/dinit.d" ];
      };
      restart = {
        on = "Failure";
        delay = 5;
      };
    };
  };
}
```

To build it manually:

```sh
cargo build --release
```

The binary is written to `target/release/sessiond-hooks`.

## Usage

```sh
sessiond-hooks [--config PATH] [--log-target stderr|syslog]
```

Example configuration:

```toml
[on_start]
exec = "/usr/bin/dinit"
args = ["--user", "--services-dir", "~/.config/dinit.d"]

[restart]
on = "Failure"
delay = 5
```
