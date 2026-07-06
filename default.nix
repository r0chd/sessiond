{
  rust-overlay ? import (import ./nix/flake-parse.nix "rust-overlay"),
  pkgs ? import (import ./nix/flake-parse.nix "nixpkgs") {
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
in
{
  shell = import ./nix/shell.nix {
    inherit pkgs rustToolchain;
  };

  nixosModules = rec {
    default = import ./nix/nixos.nix;
    sessiond = default;
  };

  finixModules = rec {
    default = import ./nix/finix.nix;
    sessiond = default;
  };

  formatter = import ./nix/formatter.nix { inherit pkgs rustToolchain; };

  packages = rec {
    default = pkgs.callPackage ./nix/package.nix {
      rustPlatform = pkgs.makeRustPlatform {
        cargo = rustToolchain;
        rustc = rustToolchain;
      };
    };
    sessiond = default;
    polkit = pkgs.callPackage ./nix/polkit {
      useConsoleKit = true;
      useSystemd = false;
    };
  };

  checks = import ./internal/finix-checks.nix { inherit finix pkgs; };
}
