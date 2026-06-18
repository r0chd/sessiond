{
  lib,
  config,
  ...
}:
let
  cfg = config.services.sessiond;
in
{
  imports = [ ./common.nix ];

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

    security.pam.services = lib.genAttrs cfg.pamServices (_: {
      rules.session.sessiond = {
        control = "optional";
        modulePath = "${cfg.package}/lib/security/pam_sessiond.so";
        order = 900;
      };
    });

    environment.systemPackages = [ cfg.package ];

    users.groups.${cfg.seat.group} = { };
  };
}
