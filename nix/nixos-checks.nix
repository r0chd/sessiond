{
  pkgs,
  self ? ../.,
  finix,
}:
let
  sessiondTest = test: pkgs.testers.nixosTest test;

  prefixAttrs = prefix: pkgs.lib.mapAttrs' (name: pkgs.lib.nameValuePair "${prefix}-${name}");

  testArgs = {
    inherit sessiondTest;
    sessiondModule = import (self + "/nix/nixos.nix");
    sessiondPackage = self.packages.${pkgs.stdenv.hostPlatform.system}.sessiond;
  };

  checks = prefixAttrs "nixos" {
    basic = pkgs.callPackage ./tests/nixos/basic.nix testArgs;
  };
in
checks
