# sessiond-uaccess

`sessiond-uaccess` is a `sessiond` client that grants device-node access to
active local sessions by writing POSIX ACL entries.

## Installation

`sessiond-uaccess` requires `sessiond` to be installed and enabled first.

On finix with flakes:

```nix
{
  inputs.sessiond-uaccess.url = "git+https://tangled.org/r0chd.pl/sessiond-uaccess";
}
```

```nix
{ inputs, pkgs, ... }:
{
  imports = [
    inputs.sessiond-uaccess.finixModules.default
  ];

  services.sessiond-uaccess = {
    enable = true;
    package = inputs.sessiond-uaccess.packages.${pkgs.system}.default;
  };
}
```

With npins:

```sh
npins add git --name sessiond-uaccess https://tangled.org/r0chd.pl/sessiond-uaccess --branch master
```

```nix
{ pkgs, ... }:
let
  sources = import ./npins;
  sessiond-uaccess = import sources.sessiond-uaccess.outPath { };
in
{
  imports = [
    sessiond-uaccess.finixModules.default
  ];

  services.sessiond-uaccess = {
    enable = true;
    package = sessiond-uaccess.packages.sessiond-uaccess;
  };
}
```

To build it manually:

```sh
cargo build --release
```

The binary is written to `target/release/sessiond-uaccess`.

## Usage

```sh
sessiond-uaccess --rules-dirs PATH [--rules-dirs PATH...]
```

`sessiond-uaccess` connects to `/run/sessiond/sessiond.sock`, loads every `*.lua` file
from each existing rules directory, scans existing udev devices, and then keeps
listening for new udev devices and session changes.

### mdevd

`sessiond-uaccess` can be used with `mdevd` and `libudev-zero`. Start `mdevd`
with `-O 4` so it rebroadcasts processed uevents to the netlink group used by
the libudev monitor:

```sh
mdevd -O 4
```

## Rules

Rules are Lua files that call `device` with an access mode and match function:

```lua
device {
  access = "rw",
  match = function(dev)
    return dev.subsystem == "sound"
  end,
}
```

The default rules in `rules/70-uaccess.lua` cover sound devices, cameras, DRI
devices, render and accelerator nodes, AMD KFD, KVM, rfkill, and optical drives.
