{
  lib,
  rustPlatform,
  pkg-config,
  udev,
  acl,
}:
let
  cargoToml = fromTOML (builtins.readFile ../../sessiond-uaccess/Cargo.toml);
in
rustPlatform.buildRustPackage (finalAttrs: {
  pname = cargoToml.package.name;
  inherit (cargoToml.package) version;

  cargoLock.lockFile = ../../sessiond-uaccess/Cargo.lock;

  buildAndTestSubdir = "sessiond-uaccess";

  postUnpack = "cp source/sessiond-uaccess/Cargo.lock source/Cargo.lock";

  src = lib.cleanSourceWith {
    src = ../..;
    filter = path: type: lib.cleanSourceFilter path type || lib.hasSuffix "/Cargo.lock" (toString path);
  };

  nativeBuildInputs = [ pkg-config ];
  buildInputs = [
    acl
    udev
  ];

  postInstall = ''
    install -d $out/share/sessiond-uaccess
    cp -r ${finalAttrs.src}/sessiond-uaccess/rules $out/share/sessiond-uaccess/
  '';

  meta = {
    description = cargoToml.package.description;
    homepage = cargoToml.package.repository;
    license = lib.licenses.gpl3Only;
    maintainers = builtins.attrValues { inherit (lib.maintainers) r0chd; };
    platforms = lib.platforms.linux;
    mainProgram = "sessiond-uaccess";
  };
})
