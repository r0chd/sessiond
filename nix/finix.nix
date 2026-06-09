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
    finit.services.sessiond = {
      description = "session management daemon";
      conditions = [ ];
      command = "${cfg.package}/bin/sessiond";
      log = true;
    };

    security.pam.services.login.text = lib.mkAfter ''
      session   optional   ${cfg.package}/lib/security/pam_sessiond.so
    '';

    environment.systemPackages = [ cfg.package ];
  };
}
