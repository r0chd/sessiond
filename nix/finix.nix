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
    finit.services.moxsession = {
      description = "session management daemon";
      conditions = [ ];
      command = "${cfg.package}/bin/moxsessiond";
    };

    security.pam.services.login.text = lib.mkAfter ''
      session   optional   ${cfg.package}/lib/security/pam_moxsession.so
    '';

    environment.systemPackages = [ cfg.package ];
  };
}
