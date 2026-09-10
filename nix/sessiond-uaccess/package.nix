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

  cargoLock = {
    lockFile = ../../Cargo.lock;
    outputHashes = {
      "pamsm-0.5.5" = "sha256-wmSKEVSgd36kwa9ky4H9+ubnqUUwyUPw/OjK8v3wGbk=";
    };
  };

  cargoBuildFlags = [
    "--package"
    cargoToml.package.name
  ];
  cargoTestFlags = [
    "--package"
    cargoToml.package.name
  ];

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
