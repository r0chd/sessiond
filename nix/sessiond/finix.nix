{
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
  # finix ships its own sessiond module; locate it through the `modules`
  # specialArg, which finix always provides (unlike `inputs`)
  disabledModules = builtins.filter (m: baseNameOf m == "sessiond") modules.default.imports;

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
      description = "modular session management daemon";
      conditions = "service/dbus/ready";
      command = "${lib.getExe' cfg.package "sessiond"} --config ${configFile} --log-target syslog";
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
