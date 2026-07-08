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
        ExecStart = "${cfg.package}/bin/sessiond --seat-user ${cfg.settings.seat.user} --seat-group ${cfg.settings.seat.group}";
        Environment = "LOG_LEVEL=info";
        Delegate = true;
      };
    };

    systemd.services.sessiond-linger-users = lib.mkIf (cfg.settings.users != { }) {
      description = "sessiond linger users";
      wantedBy = [ "multi-user.target" ];
      after = [ "sessiond.service" ];
      requires = [ "sessiond.service" ];

      serviceConfig = {
        Type = "oneshot";
        ExecStart = "${lingerUsersScript}";
        StateDirectory = "sessiond";
        WorkingDirectory = "/var/lib/sessiond";
      };
    };

    security.polkit.package = pkgs.callPackage ./polkit {
      useConsoleKit = true;
      useSystemd = false;
    };
  };
}
