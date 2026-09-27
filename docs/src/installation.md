# Installation
## Nix

{{#tabs global="installation"}}
{{#tab name="Finix"}}

```nix
{
  services.sessiond.enable = true;
}
```

{{#endtab}}
{{#tab name="NixOS + flakes"}}

```nix
{
  inputs.sessiond.url = "git+https://tangled.org/r0chd.pl/sessiond";

  outputs = { self, nixpkgs, sessiond, ... }:
    nixpkgs.lib.nixosSystem {
      modules = [
        sessiond.nixosModules.default
        { services.sessiond.enable = true; }
      ];
    };
}
```

{{#endtab}}
{{#tab name="NixOS + npins"}}

```bash
npins add git --name sessiond https://tangled.org/r0chd.pl/sessiond --branch master
```

```nix
let
  sessiond = import sources.sessiond.outPath { };
in
{
  imports = [ sessiond.nixosModules.default ];
  services.sessiond.enable = true;
}
```

{{#endtab}}
{{#endtabs}}

## From source

Build `sessiond`, `sessionctl`, and the PAM module in release mode:

```bash
cargo build -p sessiond -p sessionctl -p pam --release
```

Install the binaries and PAM module:

```bash
install -Dm755 target/release/sessiond /usr/local/bin/sessiond
install -Dm755 target/release/sessionctl /usr/local/bin/sessionctl
install -Dm755 target/release/libpam.so /usr/lib/security/pam_sessiond.so
```

The `consolekit` feature enables the ConsoleKit compatibility layer. Include it
when building if you need that support:

```bash
cargo build -p sessiond --release --features consolekit
```

## PAM

The PAM module registers login sessions with `sessiond`.

{{#tabs global="pam"}}
{{#tab name="Finix"}}

Finix has native `sessiond` support, so enabling the Finix module normally
configures the required PAM integration for you. No manual PAM configuration
is needed for the standard login service.

For a custom PAM service, add the session rule explicitly:

```nix
{
  security.pam.services.<name>.text = lib.mkAfter ''
    session   optional   ${config.services.sessiond.package}/lib/security/pam_sessiond.so
  '';
}
```

{{#endtab}}
{{#tab name="NixOS"}}

Configure the `login` PAM service as follows:

```nix
{
  security.pam.services.<name>.rules.session.sessiond = {
    control = "optional";
    modulePath = "${config.services.sessiond.package}/lib/security/pam_sessiond.so";
    order = 900;
  };
}
```

{{#endtab}}
{{#endtabs}}
