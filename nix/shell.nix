{ pkgs, rustToolchain }:
pkgs.mkShell {
  name = "moxidle-devshell";
  packages = builtins.attrValues {
    inherit
      rustToolchain
      ;
    inherit (pkgs)
      rust-analyzer-unwrapped
      nixd
      npins
      linux-pam
      ;
  };

  RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";
}
