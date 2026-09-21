{
  config,
  pkgs,
  lib,
  ...
}:
let
  cfg = config.services.sessiond-uaccess;

  configFile = pkgs.writeTextDir "00-nixos.lua" cfg.extraConfig;
in
{
  imports = [ ./common.nix ];

  config = lib.mkIf cfg.enable {
    systemd.services.sessiond-uaccess = {
      description = "grant device access to active local sessions";
      after = [ "sessiond.service" ];
      wantedBy = [ "default.target" ];

      serviceConfig = {
        ExecStart =
          "${lib.getExe cfg.package} --rules-dirs ${cfg.package}/share/sessiond-uaccess/rules "
          + lib.optionalString (cfg.extraConfig != "") "--rules-dirs ${configFile}";
        Environment = if cfg.debug then "LOG_LEVEL=debug" else "LOG_LEVEL=info";
      };
    };
  };
}
