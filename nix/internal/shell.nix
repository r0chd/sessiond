{
  pkgs,
  craneLib,
  sessiond,
  libsession,
}:
craneLib.devShell {
  name = "sessiond-devshell";
  packages = builtins.attrValues {
    inherit (pkgs)
      rust-analyzer-unwrapped
      nixd
      lon
      linux-pam
      pkg-config
      udev
      acl
      ndg
      systemdLibs
      ;
  };

  nativeBuildInputs = [ pkgs.pkg-config ];

  buildInputs = [
    sessiond
    libsession
    pkgs.rust-cbindgen
  ];

  RUST_SRC_PATH = "${pkgs.rustPlatform.rustLibSrc}";
}
