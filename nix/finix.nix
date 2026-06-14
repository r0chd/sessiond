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
    finit.services.sessiond = {
      description = "session management daemon";
      conditions = [ ];
      command = "${cfg.package}/bin/sessiond --seat-user ${cfg.seat.user} --seat-group ${cfg.seat.group}";
      log = true;
    };

    security.pam.services.login.text = lib.mkAfter ''
      session   optional   ${cfg.package}/lib/security/pam_sessiond.so
    '';

    environment.systemPackages = [ cfg.package ];

    users.groups.${cfg.seat.group} = { };
  };
}
