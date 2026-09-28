{
  inputs,
  modules,
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.services.sessiond;

  format = pkgs.formats.toml { };
  configFile = format.generate "sessiond.toml" cfg.settings;
in
{
  disabledModules = [ "${inputs.finix}/modules/services/sessiond" ];

  imports = [
    ./common.nix
    modules.polkit
  ];

  config = lib.mkIf cfg.enable {
    environment.systemPackages = [ cfg.package ];

    services.dbus = {
      enable = true;
      packages = [ cfg.package ];
    };

    services.polkit.package = lib.mkDefault (
      pkgs.polkit.override {
        useSystemd = false;
        useConsoleKit = true;
      }
    );

    finit.services.sessiond = {
      description = "daemon for power management";
      conditions = "service/dbus/ready";
      command = "${lib.getExe' cfg.package "sessiond"} --config ${configFile} --runtime-dir ${lib.escapeShellArg cfg.runtimeDir} --log-target syslog";
      notify = "systemd";
      cgroup.delegate = true;
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
