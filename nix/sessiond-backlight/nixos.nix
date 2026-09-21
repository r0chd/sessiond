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

      serviceConfig = {
        ExecStart = "${lib.getExe cfg.package}";
        Environment = if cfg.debug then "LOG_LEVEL=debug" else "LOG_LEVEL=info";
      };
    };
  };
}
