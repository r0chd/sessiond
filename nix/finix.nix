{
  lib,
  pkgs,
  config,
  ...
}:
let
  cfg = config.services.sessiond;
in
{
  imports = [ ./common.nix ];

  options.security.pam.services = lib.mkOption {
    type = lib.types.attrsOf (
      lib.types.submodule (
        { config, ... }:
        {
          options.sessiond = lib.mkEnableOption "opening sessiond sessions";

          config.text = lib.mkIf config.sessiond (
            lib.mkAfter ''
              session   optional   ${cfg.package}/lib/security/pam_sessiond.so
            ''
          );
        }
      )
    );
  };

  config = lib.mkIf cfg.enable {
    finit.services.sessiond = {
      description = "session management daemon";
      conditions = [ ];
      command = "${cfg.package}/bin/sessiond --seat-user ${cfg.seat.user} --seat-group ${cfg.seat.group}";
      log = true;
    };

    services.polkit.package = pkgs.callPackage ./polkit {
      useConsoleKit = true;
      useSystemd = false;
    };
  };
}
