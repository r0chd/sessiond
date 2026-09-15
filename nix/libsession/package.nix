{
  lib,
  stdenv,
  pkg-config,
  systemdLibs,
  rustPlatform,
  sessiondSupport ? true,
  consolekitSupport ? true,
  logindSupport ? lib.meta.availableOn stdenv.hostPlatform systemdLibs,
}:
let
  cargoToml = fromTOML (builtins.readFile ../../libsession-sys/Cargo.toml);
in
rustPlatform.buildRustPackage (finalAttrs: {
  pname = "libsession";
  inherit (cargoToml.package) version;

  outputs = [
    "out"
    "dev"
  ];

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
  ];

  buildInputs = lib.optional logindSupport systemdLibs;

  cargoBuildFlags = [
    "-p"
    "libsession-sys"
  ];
  cargoTestFlags = [
    "-p"
    "libsession-sys"
  ];

  buildNoDefaultFeatures = true;

  buildFeatures =
    lib.optional sessiondSupport "libsession-sys/sessiond"
    ++ lib.optional consolekitSupport "libsession-sys/consolekit"
    ++ lib.optional logindSupport "libsession-sys/logind";

  postInstall = ''
    install -Dm644 libsession-sys/include/libsession.h $dev/include/libsession.h

    mkdir -p $dev/lib/pkgconfig
    substitute libsession-sys/libsession.pc.in $dev/lib/pkgconfig/libsession.pc \
      --replace-fail @PREFIX@ "$out" \
      --replace-fail @INCLUDE@ "$dev/include" \
      --replace-fail @LIBDIR@ "$out/lib" \
      --replace-fail @LIBSESSION_VERSION@ "${finalAttrs.version}"
  '';

  meta = {
    outputsToInstall = [ "out" ];
    description = "Session management client library";
    homepage = "https://tangled.org/r0chd.pl/sessiond";
    license = lib.licenses.mit;
    maintainers = builtins.attrValues { inherit (lib.maintainers) r0chd; };
    platforms = lib.platforms.linux;
  };
})
