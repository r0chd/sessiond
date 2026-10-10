{
  lib,
  stdenv,
  pkg-config,
  rust-cbindgen,
  systemdLibs,
  craneLib,
  src,
  sessiondSupport ? true,
  consolekitSupport ? true,
  logindSupport ? lib.meta.availableOn stdenv.hostPlatform systemdLibs,
}:
let
  cargoToml = fromTOML (builtins.readFile ../../libsession/libsession/Cargo.toml);
  features =
    lib.optional sessiondSupport "libsession/sessiond"
    ++ lib.optional consolekitSupport "libsession/consolekit"
    ++ lib.optional logindSupport "libsession/logind";
in
craneLib.buildPackage {
  pname = "libsession";
  inherit (cargoToml.package) version;

  outputs = [
    "out"
    "dev"
  ];

  inherit src;
  strictDeps = true;

  nativeBuildInputs = [
    pkg-config
    rust-cbindgen
  ];

  buildInputs = lib.optional logindSupport systemdLibs;

  cargoExtraArgs =
    "--locked -p libsession --no-default-features"
    + lib.optionalString (features != [ ]) " --features ${lib.concatStringsSep "," features}";

  postBuild = ''
    if [ -z "''${CRANE_BUILD_DEPS_ONLY:-}" ]; then
      cbindgen --config libsession/libsession/cbindgen.toml --crate libsession --output libsession.h libsession/libsession
    fi
  '';

  postInstall = ''
    install -Dm644 libsession.h $dev/include/libsession.h

    mkdir -p $dev/lib/pkgconfig
    substitute libsession/libsession/libsession.pc.in $dev/lib/pkgconfig/libsession.pc \
      --replace-fail @PREFIX@ "$out" \
      --replace-fail @INCLUDE@ "$dev/include" \
      --replace-fail @LIBDIR@ "$out/lib" \
      --replace-fail @LIBSESSION_VERSION@ "${cargoToml.package.version}"
  '';

  meta = {
    outputsToInstall = [ "out" ];
    description = "Session management client library";
    homepage = "https://tangled.org/r0chd.pl/sessiond";
    license = lib.licenses.mit;
    maintainers = builtins.attrValues { inherit (lib.maintainers) r0chd; };
    platforms = lib.platforms.linux;
  };
}
