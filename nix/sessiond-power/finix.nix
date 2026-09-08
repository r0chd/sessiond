{
  lib,
  pkgs,
  config,
  ...
}:
let
  inherit (lib) types;
  cfg = config.services.sessiond-power;
  toml = pkgs.formats.toml { };
  configFile = toml.generate "sessiond-power.toml" cfg.settings;
in
{
  options.services.sessiond-power = {
    enable = lib.mkEnableOption "sessiond-power";
    package = lib.mkPackageOption pkgs "sessiond-power" { };
    debug = lib.mkOption {
      type = types.bool;
      default = false;
      description = ''
        Whether to enable debug logging.
      '';
    };
    settings = lib.mkOption {
      type = toml.type;
      default = { };
      example = {
        reboot = [ "/run/current-system/sw/bin/reboot" ];
        poweroff = [ "/run/current-system/sw/bin/poweroff" ];
      };
      description = "sessiond-power configuration serialized to TOML.";
    };
  };

  config = lib.mkIf cfg.enable {
    environment.systemPackages = [ cfg.package ];

    services.dbus.packages = [ cfg.package ];

    finit.services.sessiond-power = {
      description = "";
      conditions = [
        "service/sessiond/ready"
      ];
      command = "${lib.getExe cfg.package} --config ${configFile} --log-target syslog";
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
