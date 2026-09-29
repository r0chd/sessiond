# Finix

Finix has native `sessiond` support. Enable it in your system configuration:

```nix
{
  services.sessiond.enable = true;
}
```

## PAM integration

The PAM module registers login sessions with `sessiond`. Enabling the Finix
module normally configures this integration for the standard login service.

For a custom PAM service, add the session rule explicitly, replacing `<name>`
with the service name:

```nix
{ config, lib, ... }:
{
  security.pam.services.<name>.text = lib.mkAfter ''
    session   optional   ${config.services.sessiond.package}/lib/security/pam_sessiond.so
  '';
}
```
