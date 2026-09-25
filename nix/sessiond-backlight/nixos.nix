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
    systemd.services.sessiond-backlight = {
      description = "";
      after = [ "sessiond.service" ];
      wantedBy = [ "default.target" ];

      environment.SESSIOND_RUNTIME_DIR = config.services.sessiond.runtimeDir;

      serviceConfig = {
        ExecStart = "${lib.getExe cfg.package}";
        Environment = if cfg.debug then "LOG_LEVEL=debug" else "LOG_LEVEL=info";
      };
    };
  };
}
