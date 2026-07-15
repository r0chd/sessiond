# Seat Manager

Most Wayland compositors and XLibre use `libseat` to acquire seats and open
seat devices. Upstream `libseat` does not currently support `sessiond` as a
backend, so applications using the system `libseat` will not talk to
`sessiond` directly.

To bridge that gap, this project provides a `libseat` implementation that is
ABI-compatible with upstream `libseat`. Programs linked against `libseat` can
use this library to acquire seats through `sessiond`.

The `libseat` implementation lives in the separate
[`libseat-sessiond`](https://tangled.org/r0chd.pl/libseat-sessiond) repository
so it can be used as an independent input. Compositor packages such
as Hyprland and Aquamarine only need to rebuild when that implementation
changes, not when the main `sessiond` repository is updated.

`sessiond` can run alongside `seatd` when it is only used for session
management. In that setup, `seatd` remains the seat manager and compositors keep
using the system `libseat`, so no compositor packages need to be rebuilt.

## Usage

### NixOS / finix

With flakes, add `libseat-sessiond` as a separate input:

```nix
{
  inputs.libseat-sessiond.url = "git+https://tangled.org/r0chd.pl/libseat-sessiond";
}
```

Then use the pinned `libseat` package in compositor overrides:

```nix
{ pkgs, inputs, ... }:
{
  # For Hyprland, `libseat` is pulled in through Aquamarine:
  programs.hyprland.package = pkgs.hyprland.override {
    aquamarine = pkgs.aquamarine.override {
      seatd = inputs.libseat-sessiond.packages.${pkgs.stdenv.hostPlatform.system}.libseat;
    };
  };

  programs.niri.package = pkgs.niri.override {
    seatd = inputs.libseat-sessiond.packages.${pkgs.stdenv.hostPlatform.system}.libseat;
  };
}
```

With npins, pin the separate source:

```sh
npins add git --name libseat-sessiond https://tangled.org/r0chd.pl/libseat-sessiond --branch master
```

Import it from `./npins` and use the pinned `libseat` package the same way:

```nix
{ pkgs, ... }:
let
  sources = import ./npins;
  sessiondLibseat = import sources."libseat-sessiond".outPath { };
  libseat = sessiondLibseat.packages.${pkgs.stdenv.hostPlatform.system}.libseat;
in
{
  # For Hyprland, `libseat` is pulled in through Aquamarine:
  programs.hyprland.package = pkgs.hyprland.override {
    aquamarine = pkgs.aquamarine.override {
      seatd = libseat;
    };
  };

  programs.niri.package = pkgs.niri.override {
    seatd = libseat;
  };
}
```

This makes the compositor stack build against the `libseat` implementation
provided by `libseat-sessiond`, so seat acquisition and device access go
through `sessiond`.

### Other distributions

TODO
