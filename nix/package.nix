{
  withConsoleKit ? true,
  rustPlatform,
  lib,
  pkg-config,
  linux-pam,
  installShellFiles,
}:
let
  cargoToml = fromTOML (builtins.readFile ../Cargo.toml);
in
rustPlatform.buildRustPackage {
  pname = "sessiond";
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
        "ctl"
        "pam"
        "seat"
        "daemon"
        "protocols"
        "Cargo.toml"
        "Cargo.lock"
      ];
  };

  nativeBuildInputs = [
    pkg-config
    linux-pam
    installShellFiles
  ];

  cargoBuildFlags = [
    "--workspace"
  ]
  ++ lib.optionals withConsoleKit [
    "--features"
    "daemon/consolekit"
  ];

  postInstall = ''
    mkdir -p $out/lib/security
    mv $out/lib/libseat.so $out/lib/libseat.so.1
    ln -s libseat.so.1 $out/lib/libseat.so
    mv $out/lib/libpam.so $out/lib/security/pam_sessiond.so
    mv $out/bin/daemon $out/bin/sessiond
    mv $out/bin/ctl $out/bin/sessionctl

    installShellCompletion --cmd sessionctl \
      --bash <($out/bin/sessionctl completions bash) \
      --zsh <($out/bin/sessionctl completions zsh) \
      --fish <($out/bin/sessionctl completions fish) \
      --nushell <($out/bin/sessionctl completions nushell)
  ''
  + lib.optionals withConsoleKit ''
    mkdir -p $out/share/dbus-1/system.d
    cat > $out/share/dbus-1/system.d/org.freedesktop.ConsoleKit.conf <<'EOF'
    <!DOCTYPE busconfig PUBLIC
      "-//freedesktop//DTD D-BUS Bus Configuration 1.0//EN"
      "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
    <busconfig>
      <policy user="root">
        <allow own="org.freedesktop.ConsoleKit"/>
        <allow send_destination="org.freedesktop.ConsoleKit"/>
      </policy>
      <policy context="default">
        <allow send_destination="org.freedesktop.ConsoleKit"/>
      </policy>
    </busconfig>
    EOF
  '';

  meta = {
    description = "session and seat management daemon";
    homepage = "https://tangled.org/r0chd.pl/sessiond";
    license = lib.licenses.gpl3;
    maintainers = builtins.attrValues { inherit (lib.maintainers) r0chd; };
    platforms = lib.platforms.linux;
    mainProgram = "sessiond";
  };
}
