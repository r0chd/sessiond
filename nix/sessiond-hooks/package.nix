{
  lib,
  craneLib,
  src,
}:
let
  cargoToml = fromTOML (builtins.readFile ../../sessiond-hooks/Cargo.toml);
in
craneLib.buildPackage {
  pname = cargoToml.package.name;
  inherit (cargoToml.package) version;

  cargoExtraArgs = "--locked --package ${cargoToml.package.name}";

  inherit src;
  strictDeps = true;

  meta = {
    description = cargoToml.package.description;
    homepage = cargoToml.package.repository;
    license = lib.licenses.gpl3Only;
    maintainers = builtins.attrValues { inherit (lib.maintainers) r0chd; };
    platforms = lib.platforms.linux;
    mainProgram = "sessiond-hooks";
  };
}
