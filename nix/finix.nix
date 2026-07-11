{
  lib,
  pkgs,
  config,
  ...
}:
let
  cfg = config.services.sessiond;
  lingerUsersScript = import ./linger-users.nix { inherit lib pkgs cfg; };
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
    finit = {
      services.sessiond = {
        description = "session management daemon";
        conditions = [ ];
        command = "${cfg.package}/bin/sessiond --seat-user ${cfg.settings.seat.user} --seat-group ${cfg.settings.seat.group}";
        environment.LOG_LEVEL = cfg.logLevel;
        log = true;
      };
      tasks.sessiond-linger-users = lib.mkIf (cfg.settings.users != { }) {
        description = "sessiond linger users";
        conditions = [ ];
        command = "${lingerUsersScript}";
        log = true;
      };
    };

    services.polkit.package = pkgs.callPackage ./polkit {
      useConsoleKit = true;
      useSystemd = false;
    };
  };
}
