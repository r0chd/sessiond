{
  pkgs,
  self ? ../../.,
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
    sessiondModule = import (self + "/nix/sessiond/finix.nix");
    sessiondPackage = self.packages.${pkgs.stdenv.hostPlatform.system}.sessiond;
  };

  checks = prefixAttrs "finix" {
    basic = pkgs.callPackage "${self}/nix/tests/finix/basic.nix" testArgs;
    vt-switch = pkgs.callPackage "${self}/nix/tests/finix/vt-switch.nix" testArgs;
    termination = pkgs.callPackage "${self}/nix/tests/finix/termination.nix" testArgs;
    linger = pkgs.callPackage "${self}/nix/tests/finix/linger.nix" testArgs;
    consolekit = pkgs.callPackage "${self}/nix/tests/finix/consolekit.nix" testArgs;
    state-recovery = pkgs.callPackage "${self}/nix/tests/finix/state-recovery.nix" testArgs;
  };
in
checks
