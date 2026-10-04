{
  lib,
  pkgs,
  config,
  ...
}:
let
  cfg = config.services.sessiond-hooks;
  tomlFormat = pkgs.formats.toml { };
in
{
  imports = [ ./common.nix ];

  config.finit.services = lib.mapAttrs' (
    name: hook:
    let
      configFile = tomlFormat.generate "sessiond-hooks-${name}.toml" hook.settings;
    in
    lib.nameValuePair "sessiond-hooks-${name}" {
      inherit (hook) enable;
      description = "sessiond per-user hook: ${name}";
      conditions = [
        "service/sessiond/ready"
      ];
      command = lib.escapeShellArgs [
        (lib.getExe cfg.package)
        "--log-target"
        "syslog"
        "--config"
        "${configFile}"
      ];
      environment = {
        LOG_LEVEL = if cfg.debug then "debug" else lib.mkDefault "info";
      };
    }
  ) cfg.hooks;
}
