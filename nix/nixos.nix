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
    systemd.services.sessiond = {
      description = "session management daemon";
      wantedBy = [ "default.target" ];

      serviceConfig = {
        ExecStart = "${cfg.package}/bin/sessiond --config ${daemonConfig}";
        Environment = "LOG_LEVEL=${cfg.logLevel}";
        Delegate = true;
        DelegateSubgroup = "supervisor";
        RuntimeDirectory = "sessiond";
        RuntimeDirectoryMode = "0755";
        RuntimeDirectoryPreserve = "restart";
      };
    };

    security.pam.services.login.rules.session.sessiond = {
      control = "optional";
      modulePath = "${cfg.package}/lib/security/pam_sessiond.so";
      order = 900;
    };

    security.polkit.package = pkgs.callPackage ./polkit {
      useConsoleKit = true;
      useSystemd = false;
    };
  };
}
