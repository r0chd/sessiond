{
  pkgs,
  rustToolchain,
  sessiond,
}:
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
      pkg-config
      udev
      acl
      ;
  };

  nativeBuildInputs = [ pkgs.pkg-config ];
  buildInputs = [ sessiond ];

  RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";
}
