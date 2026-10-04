{
  lib,
  config,
  ...
}:
let
  cfg = config.services.sessiond-backlight;
in
{
  imports = [ ./common.nix ];

  config = lib.mkIf cfg.enable {
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
      environment = {
        LOG_LEVEL = if cfg.debug then "debug" else lib.mkDefault "info";
      };
    };
  };
}
