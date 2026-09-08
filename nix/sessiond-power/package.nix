{
  lib,
  rustPlatform,
}:
let
  workspaceToml = fromTOML (builtins.readFile ../../Cargo.toml);
  cargoToml = fromTOML (builtins.readFile ../../sessiond-power/daemon/Cargo.toml);
  cargoPackages = [
    "sessiond-power"
    "sessiond-rebootctl"
    "sessiond-poweroffctl"
    "sessiond-suspendctl"
    "sessiond-hibernatectl"
  ];
  cargoPackageFlags = builtins.concatLists (map (package: [ "--package" package ]) cargoPackages);
in
rustPlatform.buildRustPackage {
  pname = cargoToml.package.name;
  inherit (workspaceToml.workspace.package) version;

  cargoLock = {
    lockFile = ../../Cargo.lock;
    outputHashes = {
      "pamsm-0.5.5" = "sha256-wmSKEVSgd36kwa9ky4H9+ubnqUUwyUPw/OjK8v3wGbk=";
    };
  };

  cargoBuildFlags = cargoPackageFlags;
  cargoTestFlags = cargoPackageFlags;

  src = lib.cleanSource ../..;

  meta = {
    description = cargoToml.package.description;
    homepage = workspaceToml.workspace.package.repository;
    license = lib.licenses.gpl3Only;
    maintainers = builtins.attrValues { inherit (lib.maintainers) r0chd; };
    platforms = lib.platforms.linux;
    mainProgram = cargoToml.package.name;
  };
}
