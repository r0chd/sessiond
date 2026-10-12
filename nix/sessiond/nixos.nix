{
  lib,
  pkgs,
  config,
  ...
}:
let
  cfg = config.services.sessiond;
  settingsFormat = pkgs.formats.toml { };
  sessiondConfig = settingsFormat.generate "sessiond.toml" cfg.settings;
in
{
  imports = [ ./common.nix ];

  config = lib.mkIf cfg.enable {
    environment.systemPackages = [ cfg.package ];
    services.dbus.packages = [ cfg.package ];

    systemd.services.sessiond = {
      description = "modular session management daemon";
      wantedBy = [ "default.target" ];

      serviceConfig = {
        ExecStart = "${cfg.package}/bin/sessiond --config ${sessiondConfig}";
        Environment = if cfg.debug then "LOG_LEVEL=debug" else "LOG_LEVEL=info";
        Type = "notify";
        Delegate = true;
        DelegateSubgroup = "supervisor";
      };
    };

    security.polkit.package = pkgs.polkit.override {
      useConsoleKit = true;
      useSystemd = false;
    };
  };
}
