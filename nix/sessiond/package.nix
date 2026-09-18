{
  lib,
  stdenv,
  linux-pam,
  pkg-config,
  craneLib,
  src,
  withConsoleKit ? true,
  installShellFiles,
}:
let
  cargoToml = fromTOML (builtins.readFile ../../sessiond/Cargo.toml);
in
craneLib.buildPackage {
  pname = "sessiond";
  inherit (cargoToml.package) version;

  inherit src;
  strictDeps = true;

  nativeBuildInputs = [
    pkg-config
    installShellFiles
  ];

  buildInputs = [
    linux-pam
  ];

  cargoExtraArgs =
    "--locked -p sessiond -p sessionctl -p pam -p hyprwire-protocols-sessiond --no-default-features"
    + lib.optionalString withConsoleKit " --features sessiond/consolekit";
  doCheck = false;

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
