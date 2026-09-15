{
  pkgs,
  rustToolchain,
  sessiond,
  libsession,
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
      mdbook
      mdbook-plugins
      systemdLibs
      ;
  };

  nativeBuildInputs = [ pkgs.pkg-config ];
  buildInputs = [
    sessiond
    libsession
  ];

  RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";
}
