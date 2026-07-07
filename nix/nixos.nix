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

          config.rules.session.sessiond = lib.mkIf config.sessiond {
            control = "optional";
            modulePath = "${cfg.package}/lib/security/pam_sessiond.so";
            order = 900;
          };
        }
      )
    );
  };

  config = lib.mkIf cfg.enable {
    systemd.services.sessiond = {
      description = "session management daemon";
      wantedBy = [ "default.target" ];

      serviceConfig = {
        ExecStart = "${cfg.package}/bin/sessiond --seat-user ${cfg.seat.user} --seat-group ${cfg.seat.group}";
        Environment = "LOG_LEVEL=info";
        Delegate = true;
      };
    };

    security.polkit.package = pkgs.callPackage ./polkit {
      useConsoleKit = true;
      useSystemd = false;
    };
  };
}
