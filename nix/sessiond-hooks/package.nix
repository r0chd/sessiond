{
  lib,
  rustPlatform,
}:
let
  cargoToml = fromTOML (builtins.readFile ../../sessiond-hooks/Cargo.toml);
in
rustPlatform.buildRustPackage {
  pname = cargoToml.package.name;
  inherit (cargoToml.package) version;

  cargoLock.lockFile = ../../sessiond-hooks/Cargo.lock;

  buildAndTestSubdir = "sessiond-hooks";

  postUnpack = "cp source/sessiond-hooks/Cargo.lock source/Cargo.lock";

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
    mainProgram = "sessiond-hooks";
  };
}
