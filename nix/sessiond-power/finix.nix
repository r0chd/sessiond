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
    finit.services.sessiond-power = {
      description = "";
      conditions = [
        "service/sessiond/ready"
      ];
      command = "${lib.getExe cfg.package} --config ${configFile} --log-target syslog";
      environment = {
        LOG_LEVEL = if cfg.debug then "debug" else lib.mkDefault "info";
      };
    };
  };
}
