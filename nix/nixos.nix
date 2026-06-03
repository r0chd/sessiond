{
  lib,
  pkgs,
  config,
  ...
}:
let
  cfg = config.services.moxsession;
in
{
  options.services.moxsession = {
    enable = lib.mkEnableOption "moxsession";
    package = lib.mkPackageOption pkgs "moxsession" { };
  };

  config = lib.mkIf cfg.enable {
    systemd.services.moxsession = {
      description = "session management daemon";
      wantedBy = [ "default.target" ];

      serviceConfig = {
        ExecStart = "${cfg.package}/bin/moxsessiond";
        Environment = "LOG_LEVEL=info";
      };
    };

    security.pam.services.login.rules.session.moxsession = {
      control = "optional";
      modulePath = "${cfg.package}/lib/security/pam_moxsession.so";
      order = 900;
    };

    environment.systemPackages = [ cfg.package ];
  };
}
