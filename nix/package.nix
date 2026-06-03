{
  rustPlatform,
  lib,
  pkg-config,
  linux-pam,
}:
let
  cargoToml = fromTOML (builtins.readFile ../Cargo.toml);
in
rustPlatform.buildRustPackage {
  pname = "moxsession";
  inherit (cargoToml.workspace.package) version;

  cargoLock = {
    lockFile = ../Cargo.lock;
    outputHashes = {
      "pamsm-0.5.5" = "sha256-Zh6i+cMfema6wYw1eZeXnMjPOXR3z+Sdh4UUfRirH/g=";
    };
  };

  src = lib.cleanSourceWith {
    src = ../.;
    filter =
      path: type:
      let
        relPath = lib.removePrefix (toString ../. + "/") (toString path);
      in
      lib.any (p: lib.hasPrefix p relPath) [
        "daemon"
        "pam"
        "protocols"
        "Cargo.toml"
        "Cargo.lock"
      ];
  };

  nativeBuildInputs = [
    pkg-config
    linux-pam
  ];

  cargoBuildFlags = [ "--workspace" ];

  postInstall = ''
    mkdir -p $out/lib/security
    mv $out/lib/libpam.so $out/lib/security/pam_moxsession.so
    mv $out/bin/daemon $out/bin/moxsessiond
  '';

  meta = {
    description = "";
    homepage = "https://git.r0chd.pl/mox-desktop/moxsession";
    license = lib.licenses.gpl3;
    maintainers = builtins.attrValues { inherit (lib.maintainers) r0chd; };
    platforms = lib.platforms.linux;
    mainProgram = "moxsessiond";
  };
}
