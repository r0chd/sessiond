{
  rust-overlay ? import (import ./nix/internal/flake-parse.nix "rust-overlay"),
  pkgs ? import (import ./nix/internal/flake-parse.nix "nixpkgs") {
    system = builtins.currentSystem;
    overlays = [ rust-overlay ];
  },
  finix ? (import ./npins).finix,
}:
let
  rustToolchain = pkgs.rust-bin.stable.latest.default.override {
    extensions = [
      "rust-analyzer"
      "rust-src"
      "llvm-tools"
    ];
  };
  packages =
    let
      rustPlatform = pkgs.makeRustPlatform {
        cargo = rustToolchain;
        rustc = rustToolchain;
      };
    in
    rec {
      default = pkgs.callPackage ./nix/sessiond/package.nix {
        inherit rustPlatform;
      };
      sessiond = default;
      libsession = pkgs.callPackage ./nix/libsession/package.nix {
        inherit rustPlatform;
      };
      sessiond-backlight = pkgs.callPackage ./nix/sessiond-backlight/package.nix {
        inherit rustPlatform;
      };
      sessiond-hooks = pkgs.callPackage ./nix/sessiond-hooks/package.nix {
        inherit rustPlatform;
      };
      sessiond-uaccess = pkgs.callPackage ./nix/sessiond-uaccess/package.nix {
        inherit rustPlatform;
      };
      sessiond-power = pkgs.callPackage ./nix/sessiond-power/package.nix {
        inherit rustPlatform;
      };
    };
in
{
  inherit packages;

  shell = import ./nix/internal/shell.nix {
    inherit pkgs rustToolchain;
    inherit (packages) sessiond libsession;
  };

  nixosModules = rec {
    default = import ./nix/sessiond/nixos.nix;
    sessiond = default;
  };

  finixModules = rec {
    default = import ./nix/sessiond/finix.nix;
    sessiond = default;
    sessiond-backlight = import ./nix/sessiond-backlight/finix.nix;
    sessiond-hooks = import ./nix/sessiond-hooks/finix.nix;
    sessiond-uaccess = import ./nix/sessiond-uaccess/finix.nix;
    sessiond-power = import ./nix/sessiond-power/finix.nix;
  };

  formatter = import ./nix/internal/formatter.nix { inherit pkgs rustToolchain; };

  checks =
    (import ./nix/finix-checks.nix { inherit finix pkgs; })
    // (import ./nix/nixos-checks.nix { inherit finix pkgs; });
}
