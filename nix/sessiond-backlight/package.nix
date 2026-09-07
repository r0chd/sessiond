{
  lib,
  rustPlatform,
}:
let
  cargoToml = fromTOML (builtins.readFile ../../sessiond-backlight/Cargo.toml);
in
rustPlatform.buildRustPackage {
  pname = cargoToml.package.name;
  inherit (cargoToml.package) version;

  cargoLock.lockFile = ../../sessiond-backlight/Cargo.lock;

  buildAndTestSubdir = "sessiond-backlight";

  postUnpack = "cp source/sessiond-backlight/Cargo.lock source/Cargo.lock";

  src = lib.cleanSourceWith {
    src = ../..;
    filter = path: type: lib.cleanSourceFilter path type || lib.hasSuffix "/Cargo.lock" (toString path);
  };

  meta = {
    description = cargoToml.package.description;
    homepage = cargoToml.package.repository;
    license = lib.licenses.gpl3Only;
    maintainers = builtins.attrValues { inherit (lib.maintainers) r0chd; };
    platforms = lib.platforms.linux;
    mainProgram = "sessiond-backlight";
  };
}
