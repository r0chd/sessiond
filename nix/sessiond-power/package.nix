{
  lib,
  craneLib,
  src,
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
    "sessiond-hybrid-sleepctl"
  ];
  cargoPackageFlags = builtins.concatLists (
    map (package: [
      "--package"
      package
    ]) cargoPackages
  );
in
craneLib.buildPackage {
  pname = cargoToml.package.name;
  version = "0.0.0";

  cargoExtraArgs = lib.escapeShellArgs ([ "--locked" ] ++ cargoPackageFlags);

  inherit src;
  strictDeps = true;

  meta = {
    description = cargoToml.package.description;
    homepage = workspaceToml.workspace.package.repository;
    license = lib.licenses.gpl3Only;
    maintainers = builtins.attrValues { inherit (lib.maintainers) r0chd; };
    platforms = lib.platforms.linux;
    mainProgram = cargoToml.package.name;
  };
}
