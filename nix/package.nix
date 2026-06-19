{
  rustPlatform,
  lib,
  pkg-config,
  linux-pam,
  installShellFiles,
}:
let
  cargoToml = fromTOML (builtins.readFile ../Cargo.toml);
in
rustPlatform.buildRustPackage {
  pname = "sessiond";
  inherit (cargoToml.workspace.package) version;

  cargoLock = {
    lockFile = ../Cargo.lock;
    outputHashes = {
      "pamsm-0.5.5" = "sha256-Zh6i+cMfema6wYw1eZeXnMjPOXR3z+Sdh4UUfRirH/g=";
    };
  };

  src = lib.cleanSourceWith {
    src = ../.;
    filter =
      path: type:
      let
        relPath = lib.removePrefix (toString ../. + "/") (toString path);
      in
      lib.any (p: lib.hasPrefix p relPath) [
        "ctl"
        "pam"
        "daemon"
        "protocols"
        "Cargo.toml"
        "Cargo.lock"
      ];
  };

  nativeBuildInputs = [
    pkg-config
    linux-pam
    installShellFiles
  ];

  cargoBuildFlags = [ "--workspace" ];

  postInstall = ''
    mkdir -p $out/lib/security
    mv $out/lib/libpam.so $out/lib/security/pam_sessiond.so
    mv $out/bin/daemon $out/bin/sessiond
    mv $out/bin/ctl $out/bin/sessionctl

    installShellCompletion --cmd sessionctl \
      --bash <($out/bin/sessionctl completions bash) \
      --zsh <($out/bin/sessionctl completions zsh) \
      --fish <($out/bin/sessionctl completions fish) \
      --nushell <($out/bin/sessionctl completions nushell)
  '';

  meta = {
    description = "";
    homepage = "https://tangled.org/r0chd.pl/sessiond";
    license = lib.licenses.gpl3;
    maintainers = builtins.attrValues { inherit (lib.maintainers) r0chd; };
    platforms = lib.platforms.linux;
    mainProgram = "sessiond";
  };
}
