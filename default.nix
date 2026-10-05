{
  pkgs ? import (import ./nix/internal/flake-parse.nix "nixpkgs") {
    system = builtins.currentSystem;
  },
  craneLib ? import (import ./nix/internal/flake-parse.nix "crane") { inherit pkgs; },
  finix ? (import ./lon.nix).finix,
}:
let
  src = pkgs.lib.cleanSourceWith {
    src = pkgs.lib.cleanSource ./.;
    name = "sessiond-source";
    filter =
      path: type:
      !(
        type == "directory"
        && builtins.elem (baseNameOf path) [
          "target"
          ".direnv"
        ]
      )
      && (
        craneLib.filterCargoSources path type
        || pkgs.lib.any (suffix: pkgs.lib.hasSuffix suffix path) [
          ".xml"
          ".lua"
          ".h"
          ".pc.in"
          ".conf"
          ".md"
        ]
        || baseNameOf path == "LICENSE"
        || baseNameOf path == "cbindgen.toml"
      );
  };
  callPackage = path: pkgs.callPackage path { inherit craneLib src; };
  packages = rec {
    default = sessiond;
    sessiond = callPackage ./nix/sessiond/package.nix;
    libsession = callPackage ./nix/libsession/package.nix;
    sessiond-backlight = callPackage ./nix/sessiond-backlight/package.nix;
    sessiond-hooks = callPackage ./nix/sessiond-hooks/package.nix;
    sessiond-uaccess = callPackage ./nix/sessiond-uaccess/package.nix;
    sessiond-power = callPackage ./nix/sessiond-power/package.nix;
  };
  checkArgs = {
    inherit finix pkgs;
    self = {
      outPath = ./.;
      packages.${pkgs.stdenv.hostPlatform.system} = packages;
    };
  };
in
{
  inherit packages;

  shell = import ./nix/internal/shell.nix {
    inherit pkgs craneLib;
    inherit (packages) sessiond libsession;
  };

  nixosModules = rec {
    default = import ./nix/sessiond/nixos.nix;
    sessiond = default;
    sessiond-backlight = import ./nix/sessiond-backlight/nixos.nix;
    sessiond-hooks = import ./nix/sessiond-hooks/nixos.nix;
    sessiond-uaccess = import ./nix/sessiond-uaccess/nixos.nix;
    sessiond-power = import ./nix/sessiond-power/nixos.nix;
  };

  finixModules = rec {
    default = import ./nix/sessiond/finix.nix;
    sessiond = default;
    sessiond-backlight = import ./nix/sessiond-backlight/finix.nix;
    sessiond-hooks = import ./nix/sessiond-hooks/finix.nix;
    sessiond-uaccess = import ./nix/sessiond-uaccess/finix.nix;
    sessiond-power = import ./nix/sessiond-power/finix.nix;
  };

  formatter = import ./nix/internal/formatter.nix { inherit pkgs; };

  checks =
    (import ./nix/internal/finix-checks.nix checkArgs)
    // (import ./nix/internal/nixos-checks.nix checkArgs);
}
