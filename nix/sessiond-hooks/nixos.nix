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

  systemd.services = lib.mapAttrs' (
    name: hook:
    let
      configFile = tomlFormat.generate "sessiond-hooks-${name}.toml" hook.settings;
    in
    lib.nameValuePair "sessiond-hooks-${name}" {
      inherit (hook) enable;
      description = "sessiond per-user hook: ${name}";
      after = [ "sessiond.service" ];
      wantedBy = [ "default.target" ];

      serviceConfig = {
        ExecStart = "${lib.getExe cfg.package} --config ${configFile}";
        Environment = if cfg.debug then "LOG_LEVEL=debug" else "LOG_LEVEL=info";
      };
    }
  ) cfg.hooks;
}
