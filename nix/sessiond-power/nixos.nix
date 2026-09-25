{
  lib,
  pkgs,
  config,
  ...
}:
let
  cfg = config.services.sessiond-power;
  toml = pkgs.formats.toml { };
  configFile = toml.generate "sessiond-power.toml" cfg.settings;
in
{
  imports = [ ./common.nix ];

  config = lib.mkIf cfg.enable {
    systemd.services.sessiond-power = {
      description = "";
      after = [ "sessiond.service" ];
      wantedBy = [ "default.target" ];

      environment.SESSIOND_RUNTIME_DIR = config.services.sessiond.runtimeDir;

      serviceConfig = {
        ExecStart = "${lib.getExe cfg.package} --config ${configFile}";
        Environment = if cfg.debug then "LOG_LEVEL=debug" else "LOG_LEVEL=info";
      };
    };
  };
}
