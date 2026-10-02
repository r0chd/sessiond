# NixOS

`sessiond` is not officially supported on NixOS and requires additional manual
configuration. NixOS is deeply integrated with `logind`, which `sessiond`
replaces, so some system components may continue to depend on `logind` and need
separate adjustment.

## Install the module

Choose your prefered way of pinning the source.

### Flakes

Add the input to your flake and import its module in your NixOS configuration:

```nix
{
  inputs.sessiond.url = "git+https://tangled.org/r0chd.pl/sessiond";

  outputs = { self, nixpkgs, sessiond, ... }: {
    nixosConfigurations.myHost = nixpkgs.lib.nixosSystem {
      modules = [
        ./configuration.nix
        sessiond.nixosModules.default
        { services.sessiond.enable = true; }
      ];
    };
  };
}
```

### npins

```bash
npins add git --name sessiond https://tangled.org/r0chd.pl/sessiond --branch master
```

```nix
let
  sources = import ./npins;
  sessiond = import sources.sessiond.outPath { };
in
{
  imports = [ sessiond.nixosModules.default ];
  services.sessiond.enable = true;
}
```

## Using sessiond in place of logind

Enabling the module does not disable `logind`. Running both services causes
conflicts, each attempts to move the session leader process, such as a greeter,
into its own cgroup. Moving the process out of the other service's cgroup
disrupts its session tracking. Disable `logind` when using `sessiond`.

```nix
{ pkgs, ... }:
{
  services.logind.enable = false;
}
```

This keeps `systemd` as the system service manager but removes ability for `systemd`
to manage user services. You also lose ability to view `systemd` service status without
root permissions

## PAM integration

The module starts the daemon but does not add the PAM session rule. To register
console logins, configure the `login` PAM service:

```nix
{ config, ... }: {
  security.pam.services.login.rules.session.sessiond = {
    control = "optional";
    modulePath = "${config.services.sessiond.package}/lib/security/pam_sessiond.so";
    order = 900;
  };
}
```

For other login paths, add the rule to the corresponding PAM service as well.
Configuring `login` alone does not configure a display manager or SSH.

## Seat management

`sessiond` does not provide seat management on its own. To allow your compositor
to access graphical devices, enable `seatd` and add your user to the `seat` group.

```nix
{ config, ... }: {
  services.seatd.enable = true;

  users.users.myUser.extraGroups = [ config.services.seatd.group ];
}
```

## Greeters/Display Managers

To allow the greeter to access graphical devices, add the user under which the
service runs to the `seat` group. Also ensure that `greetd` starts after both
`seatd` and `sessiond` to avoid race conditions.

```nix
{ config, ... }: {
  users.users.cosmic-greeter.extraGroups =  [
    config.services.seatd.group
  ];
  
  systemd.services.greetd = {
    wants = [ "seatd.service" "sessiond.service" ];
    after = [ "seatd.service" "sessiond.service" ];
  };
}
```

### switch-to-configuration

With `sessiond`, you have two options when applying a NixOS configuration:

- Use `nixos-rebuild boot` and reboot.
- Use [my fork of `switch-to-configuration-ng`](https://tangled.org/r0chd.pl/switch-to-configuration-ng) to switch without rebooting.

  ```nix
  # In your flake's inputs:
  inputs.switch-to-configuration.url =
    "git+https://tangled.org/r0chd.pl/switch-to-configuration-ng";

  # In a NixOS module:
  { inputs, ... }: {
    nixpkgs.overlays = [
      (final: prev: {
        switch-to-configuration-ng = inputs.switch-to-configuration.packages.default;
      })
    ];
  }
  ```

To use `switch-to-configuration-ng`, the user systemd service manager needs to
be running. The recommended way is to use `sessiond-hooks` to start and
supervise it:

```nix
{ inputs, pkgs, ... }: {
  imports = [ inputs.sessiond.nixosModules.sessiond-hooks ];

  services.sessiond-hooks = {
    package = inputs.sessiond.packages.sessiond-hooks;
    debug = true;
    hooks.systemd.settings = {
      on_start = {
        exec = "${pkgs.systemd}/lib/systemd/systemd";
        args = [ "--user" ];
      };
      restart = {
        on = "Always";
        delay = 5;
      };
    };
  };
}
```
