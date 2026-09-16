{
  lib,
  stdenv,
  linux-pam,
  pkg-config,
  rustPlatform,
  withConsoleKit ? true,
  installShellFiles,
}:
let
  cargoToml = fromTOML (builtins.readFile ../../sessiond/Cargo.toml);
in
rustPlatform.buildRustPackage {
  pname = "sessiond";
  inherit (cargoToml.package) version;

  cargoLock = {
    lockFile = ../../Cargo.lock;
    outputHashes = {
      "pamsm-0.5.5" = "sha256-wmSKEVSgd36kwa9ky4H9+ubnqUUwyUPw/OjK8v3wGbk=";
    };
  };

  src = lib.cleanSourceWith {
    src = ../..;
    filter =
      path: type:
      let
        relPath = lib.removePrefix (toString ../.. + "/") (toString path);
      in
      lib.any (p: lib.hasPrefix p relPath) [
        "sessionctl"
        "pam"
        "sessiond"
        "libsession-sys"
        "libsession-rs"
        "data"
        "protocols"
        "hyprwire-protocols-sessiond"
        "Cargo.toml"
        "Cargo.lock"
      ];
  };

  nativeBuildInputs = [
    pkg-config
    installShellFiles
  ];

  buildInputs = [
    linux-pam
  ];

  cargoBuildFlags = [
    "-p"
    "sessiond"
    "-p"
    "sessionctl"
    "-p"
    "pam"
    "-p"
    "hyprwire-protocols-sessiond"
  ];

  buildNoDefaultFeatures = true;
  doCheck = false;

  buildFeatures = lib.optional withConsoleKit "sessiond/consolekit";

  postInstall = ''
    mkdir -p $out/lib/security
    mv $out/lib/libpam.so $out/lib/security/pam_sessiond.so
  ''
  + lib.optionalString (stdenv.buildPlatform.canExecute stdenv.hostPlatform) ''
    installShellCompletion --cmd sessionctl \
      --bash <($out/bin/sessionctl completions bash) \
      --zsh <($out/bin/sessionctl completions zsh) \
      --fish <($out/bin/sessionctl completions fish) \
      --nushell <($out/bin/sessionctl completions nushell)
  ''
  + lib.optionalString withConsoleKit ''
    install -Dm644 data/dbus-1/system.d/org.freedesktop.ConsoleKit.conf \
      $out/share/dbus-1/system.d/org.freedesktop.ConsoleKit.conf
  '';

  meta = {
    outputsToInstall = [ "out" ];
    description = cargoToml.package.description;
    homepage = cargoToml.package.repository;
    license = lib.licenses.gpl3Only;
    maintainers = builtins.attrValues { inherit (lib.maintainers) r0chd; };
    platforms = lib.platforms.linux;
  };
}
