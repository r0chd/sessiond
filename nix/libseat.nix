{
  rustPlatform,
  lib,
}:
let
  cargoToml = fromTOML (builtins.readFile ../seat/Cargo.toml);
  libseatVersion = "0.9.3";
in
rustPlatform.buildRustPackage {
  pname = "libseat-sessiond";
  inherit (cargoToml.package) version;

  cargoLock.lockFile = ../seat/Cargo.lock;

  src = lib.cleanSourceWith {
    src = ../.;
    filter =
      path: type:
      let
        relPath = lib.removePrefix (toString ../. + "/") (toString path);
      in
      lib.any (p: lib.hasPrefix p relPath) [
        "seat"
        "protocols"
        "README.md"
      ];
  };

  sourceRoot = "source/seat";

  postInstall = ''
    mv $out/lib/libseat.so $out/lib/libseat.so.1
    ln -s libseat.so.1 $out/lib/libseat.so
    install -Dm644 include/libseat.h $out/include/libseat.h
    mkdir -p $out/lib/pkgconfig
    cat > $out/lib/pkgconfig/libseat.pc <<EOF
    prefix=$out
    includedir=''${prefix}/include
    libdir=''${prefix}/lib

    Name: libseat
    Description: Seat management library
    Version: ${libseatVersion}
    Libs: -L''${libdir} -lseat
    Cflags: -I''${includedir}
    EOF
  '';

  meta = {
    description = "libseat-compatible client library for sessiond";
    homepage = "https://tangled.org/r0chd.pl/sessiond";
    license = lib.licenses.gpl3;
    maintainers = builtins.attrValues { inherit (lib.maintainers) r0chd; };
    platforms = lib.platforms.linux;
  };
}
