# NixOS

The NixOS module runs `sessiond` as a systemd service. PAM integration and the
choice of session manager need additional configuration, described below.

## Install the module

Choose either flakes or npins to import the module.

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
{ config, ... }:
{
  security.pam.services.login.rules.session.sessiond = {
    control = "optional";
    modulePath = "${config.services.sessiond.package}/lib/security/pam_sessiond.so";
    order = 900;
  };
}
```

For other login paths, add the rule to the corresponding PAM service as well.
Configuring `login` alone does not configure a display manager or SSH.

## Known issues

The following problems remain unresolved in this setup.

### greetd greeter

The greetd greeter fails to start, leaving a black screen. The cause has not
yet been identified.

### Switching configurations

`switch-to-configuration` does not currently work with this setup for two reasons:

- It uses `logind` to list active users.
- It relies on per-user systemd service managers to start
  `nixos-activation.service`.

Both dependencies need to be addressed for configuration switching to work
with `sessiond` in place of `logind`.

For now, use `nixos-rebuild boot` instead of `nixos-rebuild switch` to build the
configuration and make it the default for the next boot, then reboot to apply
it. Include your usual flake or configuration arguments:

```bash
sudo nixos-rebuild boot --flake .#myHost
sudo reboot
```
