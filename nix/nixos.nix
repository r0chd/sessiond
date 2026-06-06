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
  options.services.sessiond = {
    enable = lib.mkEnableOption "sessiond";
    package = lib.mkPackageOption pkgs "sessiond" { };
  };

  config = lib.mkIf cfg.enable {
    systemd.services.sessiond = {
      description = "session management daemon";
      wantedBy = [ "default.target" ];

      serviceConfig = {
        ExecStart = "${cfg.package}/bin/sessiond";
        Environment = "LOG_LEVEL=info";
      };
    };

    security.pam.services.login.rules.session.sessiond = {
      control = "optional";
      modulePath = "${cfg.package}/lib/security/pam_sessiond.so";
      order = 900;
    };

    environment.systemPackages = [ cfg.package ];
  };
}
