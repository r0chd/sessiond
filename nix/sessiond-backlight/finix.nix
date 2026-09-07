{
  lib,
  pkgs,
  config,
  ...
}:
let
  inherit (lib) types;
  cfg = config.services.sessiond-backlight;
in
{
  options.services.sessiond-backlight = {
    enable = lib.mkEnableOption "sessiond-backlight";
    package = lib.mkPackageOption pkgs "sessiond-backlight" { };
    debug = lib.mkOption {
      type = types.bool;
      default = false;
      description = ''
        Whether to enable debug logging.
      '';
    };
  };

  config = lib.mkIf cfg.enable {
    services.dbus.packages = [ cfg.package ];

    finit.services.sessiond-backlight = {
      description = "";
      conditions = [
        "service/sessiond/ready"
      ];
      command = lib.escapeShellArgs [
        (lib.getExe cfg.package)
        "--log-target"
        "syslog"
      ];
      environment =
        if cfg.debug then
          {
            LOG_LEVEL = "debug";
          }
        else
          {
            LOG_LEVEL = lib.mkDefault "info";
          };
    };
  };
}
