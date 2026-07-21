{
  pkgs,
  self ? ../.,
  finix,
}:
let
  testLib = import (finix + "/tests/lib") {
    inherit pkgs;
    inherit (pkgs) lib;
  };
  sessiondTest = test: testLib.mkTest test;

  prefixAttrs = prefix: pkgs.lib.mapAttrs' (name: pkgs.lib.nameValuePair "${prefix}-${name}");

  testArgs = {
    inherit sessiondTest;
    sessiondModule = import (self + "/nix/finix.nix");
    sessiondPackage = self.packages.${pkgs.stdenv.hostPlatform.system}.sessiond;
  };

  checks = prefixAttrs "finix" {
    basic = pkgs.callPackage ./tests/finix/basic.nix testArgs;
    vt-switch = pkgs.callPackage ./tests/finix/vt-switch.nix testArgs;
    termination = pkgs.callPackage ./tests/finix/termination.nix testArgs;
    linger = pkgs.callPackage ./tests/finix/linger.nix testArgs;
    consolekit = pkgs.callPackage ./tests/finix/consolekit.nix testArgs;
  };
in
checks
