{
  lib,
  pkgs,
  config,
  ...
}:
let
  inherit (lib) types;
  cfg = config.services.sessiond-hooks;
  tomlFormat = pkgs.formats.toml { };
in
{
  options.services.sessiond-hooks = {
    package = lib.mkPackageOption pkgs "sessiond-hooks" { };
    debug = lib.mkOption {
      type = types.bool;
      default = false;
      description = ''
        Whether to enable debug logging.
      '';
    };
    hooks = lib.mkOption {
      type = types.attrsOf (
        types.submodule {
          options = {
            enable = lib.mkOption {
              type = types.bool;
              default = true;
            };
            settings = lib.mkOption {
              type = tomlFormat.type;
              description = "Hook settings serialized to TOML.";
            };
          };
        }
      );
      default = { };
      description = ''
        Per-user session hooks launched. Each named entry spawns an
        independent `sessiond-hooks` process.      '';
    };
  };

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
      environment =
        if cfg.debug then
          {
            LOG_LEVEL = "debug";
          }
        else
          {
            LOG_LEVEL = lib.mkDefault "info";
          };
    }
  ) cfg.hooks;
}
