{
  lib,
  craneLib,
  src,
  pkg-config,
  udev,
  acl,
}:
let
  cargoToml = fromTOML (builtins.readFile ../../sessiond-uaccess/Cargo.toml);
in
craneLib.buildPackage {
  pname = cargoToml.package.name;
  inherit (cargoToml.package) version;

  cargoExtraArgs = "--locked --package ${cargoToml.package.name}";

  inherit src;
  strictDeps = true;

  nativeBuildInputs = [ pkg-config ];
  buildInputs = [
    acl
    udev
  ];

  postInstall = ''
    install -Dm644 -t $out/share/sessiond-uaccess/rules sessiond-uaccess/rules/*.lua
  '';

  meta = {
    description = cargoToml.package.description;
    homepage = cargoToml.package.repository;
    license = lib.licenses.gpl3Only;
    maintainers = builtins.attrValues { inherit (lib.maintainers) r0chd; };
    platforms = lib.platforms.linux;
    mainProgram = "sessiond-uaccess";
  };
}
