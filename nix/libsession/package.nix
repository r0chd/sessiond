{
  lib,
  stdenv,
  pkg-config,
  systemdLibs,
  craneLib,
  src,
  sessiondSupport ? true,
  consolekitSupport ? true,
  logindSupport ? lib.meta.availableOn stdenv.hostPlatform systemdLibs,
}:
let
  cargoToml = fromTOML (builtins.readFile ../../libsession-sys/Cargo.toml);
  features =
    lib.optional sessiondSupport "libsession-sys/sessiond"
    ++ lib.optional consolekitSupport "libsession-sys/consolekit"
    ++ lib.optional logindSupport "libsession-sys/logind";
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
  ];

  buildInputs = lib.optional logindSupport systemdLibs;

  cargoExtraArgs =
    "--locked -p libsession-sys --no-default-features"
    + lib.optionalString (features != [ ]) " --features ${lib.concatStringsSep "," features}";

  postInstall = ''
    install -Dm644 libsession-sys/include/libsession.h $dev/include/libsession.h

    mkdir -p $dev/lib/pkgconfig
    substitute libsession-sys/libsession.pc.in $dev/lib/pkgconfig/libsession.pc \
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
