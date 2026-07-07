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

`sessiond` can also work with polkit through its ConsoleKit
backend. By default, the polkit package in nixpkgs doesn't
have ConsoleKit support enabled, so `sessiond` provides its
own polkit package with it built in.

```nix
{
  security.polkit.package = inputs.sessiond.packages.${pkgs.stdenv.hostPlatform.system}.polkit;
}
```

or on finix

```nix
{
  services.polkit.package = inputs.sessiond.packages.${pkgs.stdenv.hostPlatform.system}.polkit;
}
```

## Other distributions

TODO
