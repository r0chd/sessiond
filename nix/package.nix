{
  lib,
  stdenv,
  linux-pam,
  pkg-config,
  rustPlatform,
  withConsoleKit ? true,
  libsessionBackends ? [
    "sessiond"
    "logind"
    "consolekit"
  ],
  installShellFiles,
}:
let
  cargoToml = fromTOML (builtins.readFile ../Cargo.toml);

  # libsession-sys/libsession-rs link against libsession, which
  # only exists once this derivation's own postInstall runs
  cargoWorkspaceFlags = [
    "--workspace"
    "--exclude"
    "libsession-sys"
    "--exclude"
    "libsession-rs"
  ];
in
rustPlatform.buildRustPackage {
  pname = "sessiond";
  inherit (cargoToml.workspace.package) version;

  outputs = [
    "out"
    "dev"
    "lib"
  ];

  cargoLock = {
    lockFile = ../Cargo.lock;
    outputHashes = {
      "pamsm-0.5.5" = "sha256-wmSKEVSgd36kwa9ky4H9+ubnqUUwyUPw/OjK8v3wGbk=";
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
        "daemon"
        "libsession"
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

  cargoBuildFlags = cargoWorkspaceFlags;
  cargoTestFlags = cargoWorkspaceFlags;

  buildNoDefaultFeatures = true;

  buildFeatures =
    lib.optionals withConsoleKit [
      "daemon/consolekit"
    ]
    ++ map (backend: "libsession/${backend}") libsessionBackends;

  postInstall = ''
    mkdir -p $out/lib/security
    mv $out/lib/libpam.so $out/lib/security/pam_sessiond.so
    mv $out/bin/daemon $out/bin/sessiond
    mv $out/bin/ctl $out/bin/sessionctl

    install -Dm644 libsession/include/libsession.h $out/include/libsession.h

    mkdir -p $out/lib/pkgconfig
    cat > $out/lib/pkgconfig/libsession.pc <<EOF
    prefix=$lib
    libdir=''${prefix}/lib
    includedir=$dev/include

    Name: libsession
    Description: C ABI session management client library
    Version: ${cargoToml.workspace.package.version}
    Libs: -L''${libdir} -lsession
    Cflags: -I''${includedir}
    EOF

    moveToOutput lib/libsession.so $lib
    moveToOutput lib/libsession.a $lib
    moveToOutput include $dev
    moveToOutput lib/pkgconfig $dev
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
    description = "Session management daemon";
    homepage = "https://tangled.org/r0chd.pl/sessiond";
    license = lib.licenses.gpl3Only;
    maintainers = builtins.attrValues { inherit (lib.maintainers) r0chd; };
    platforms = lib.platforms.linux;
  };
}
