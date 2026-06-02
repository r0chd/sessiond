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
      pkg-config
      libpulseaudio
      ;
  };

  buildInputs = [
    pkgs.libpulseaudio
    pkgs.pkg-config
  ];

  RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";
}
