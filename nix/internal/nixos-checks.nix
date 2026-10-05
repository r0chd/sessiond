{
  pkgs,
  self ? ../../.,
  ...
}:
let
  sessiondPackage = self.packages.${pkgs.stdenv.hostPlatform.system}.sessiond;
  sessiondTest =
    test:
    pkgs.testers.nixosTest (
      test
      // {
        nodes = pkgs.lib.mapAttrs (_: node: {
          imports = [
            node
            {
              services.logind.enable = false;
              security.pam.services.login.rules.session.sessiond = {
                control = "optional";
                modulePath = "${sessiondPackage}/lib/security/pam_sessiond.so";
                order = 900;
              };
            }
          ];
        }) test.nodes;
      }
    );

  prefixAttrs = prefix: pkgs.lib.mapAttrs' (name: pkgs.lib.nameValuePair "${prefix}-${name}");

  testArgs = {
    inherit sessiondTest;
    sessiondModule = import (self + "/nix/sessiond/nixos.nix");
    inherit sessiondPackage;
  };

  checks = prefixAttrs "nixos" {
    basic = pkgs.callPackage "${self}/nix/tests/nixos/basic.nix" testArgs;
  };
in
checks
