# Seat Manager

Most Wayland compositors and XLibre use `libseat` to acquire seats and open
seat devices. Upstream `libseat` does not currently support `sessiond` as a
backend, so applications using the system `libseat` will not talk to
`sessiond` directly.

To bridge that gap, this project provides a `libseat` implementation that is
ABI-compatible with upstream `libseat`. Programs linked against `libseat` can
use this library to acquire seats through `sessiond`.

`sessiond` can run alongside `seatd` when it is only used for session
management. In that setup, `seatd` remains the seat manager and compositors keep
using the system `libseat`, so no compositor packages need to be rebuilt.

## Usage

### NixOS / finix

```nix
{ pkgs, inputs, ... }:
{

  # For Hyprland, `libseat` is pulled in through Aquamarine:
  programs.hyprland.package = pkgs.hyprland.override {
    aquamarine = pkgs.aquamarine.override {
      seatd = inputs.sessiond.packages.${pkgs.stdenv.hostPlatform.system}.libseat;
    };
  };

  programs.niri.package = pkgs.niri.override {
    seatd = inputs.sessiond.packages.${pkgs.stdenv.hostPlatform.system}.libseat;
  };
}
```

This makes the compositor stack build against the `libseat` implementation
provided by `sessiond`, so seat acquisition and device access go through
`sessiond`.

> These overrides cause cache misses for the affected packages, so they need to
> be built locally.

### Other distributions

TODO
