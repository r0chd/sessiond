{
  lib,
  pkgs,
  config,
  ...
}:
let
  cfg = config.services.sessiond;
  settingsFormat = pkgs.formats.toml { };
  daemonConfig = settingsFormat.generate "sessiond.toml" cfg.settings;
in
{
  imports = [ ./common.nix ];

  config = lib.mkIf cfg.enable {
    finit = {
      services.sessiond = {
        description = "session management daemon";
        conditions = [ ];
        command = "${cfg.package}/bin/sessiond --config ${daemonConfig}";
        environment.LOG_LEVEL = cfg.logLevel;
        log = true;
      };
    };

    security.pam.services.login.text = lib.mkAfter ''
      session   optional   ${cfg.package}/lib/security/pam_sessiond.so
    '';

    services.polkit.package = pkgs.callPackage ./polkit {
      useConsoleKit = true;
      useSystemd = false;
    };
  };
}
