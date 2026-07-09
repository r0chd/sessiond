{ pkgs, rustToolchain }:
pkgs.mkShell {
  name = "sessiond-devshell";
  packages = builtins.attrValues {
    inherit
      rustToolchain
      ;
    inherit (pkgs)
      rust-analyzer-unwrapped
      nixd
      npins
      linux-pam

      seatd
      pkg-config
      libabigail
      ;
  };

  nativeBuildInputs = [ pkgs.pkg-config ];

  RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";
}
