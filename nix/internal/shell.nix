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

  RUST_SRC_PATH = "${pkgs.rustPlatform.rustLibSrc}";
}
