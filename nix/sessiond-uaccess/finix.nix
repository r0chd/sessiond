{
  lib,
  pkgs,
  config,
  ...
}:
let
  cfg = config.services.sessiond-uaccess;

  configFile = pkgs.writeTextDir "00-nixos.lua" cfg.extraConfig;

  # gardendevd needs libudev-garden; mdevd/keventd need libudev-zero
  udevApi =
    if config.services.gardendevd.enable then
      pkgs.libudev-garden
    else if config.services.mdevd.enable || config.services.keventd.enable then
      pkgs.libudev-zero
    else
      null;
in
{
  imports = [ ./common.nix ];

  config = lib.mkIf cfg.enable {
    services.sessiond-uaccess.package = lib.mkDefault (
      pkgs.sessiond-uaccess.override (
        lib.optionalAttrs (udevApi != null) {
          udev = udevApi;
        }
      )
    );

    finit.services.sessiond-uaccess = {
      description = "grant device access to active local sessions";
      conditions = "service/sessiond/ready";
      command =
        "${lib.getExe cfg.package} --log-target syslog --rules-dirs ${cfg.package}/share/sessiond-uaccess/rules "
        + lib.optionalString (cfg.extraConfig != "") "--rules-dirs ${configFile}";
      environment = {
        LOG_LEVEL = if cfg.debug then "debug" else lib.mkDefault "info";
      };
    };
  };
}
