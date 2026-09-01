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
      default = pkgs.callPackage ./nix/package.nix {
        inherit rustPlatform;
      };
      sessiond = default;
    };
in
{
  inherit packages;

  shell = import ./nix/internal/shell.nix {
    inherit pkgs rustToolchain;
    inherit (packages) sessiond;
  };

  nixosModules = rec {
    default = import ./nix/nixos.nix;
    sessiond = default;
  };

  finixModules = rec {
    default = import ./nix/finix.nix;
    sessiond = default;
  };

  formatter = import ./nix/internal/formatter.nix { inherit pkgs rustToolchain; };

  checks =
    (import ./nix/finix-checks.nix { inherit finix pkgs; })
    // (import ./nix/nixos-checks.nix { inherit finix pkgs; });
}
