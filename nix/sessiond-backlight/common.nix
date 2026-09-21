{
  lib,
  pkgs,
  ...
}:
let
  inherit (lib) types;
in
{
  options.services.sessiond-backlight = {
    enable = lib.mkEnableOption "sessiond-backlight";
    package = lib.mkPackageOption pkgs "sessiond-backlight" { };
    debug = lib.mkOption {
      type = types.bool;
      default = false;
      description = ''
        Whether to enable debug logging.
      '';
    };
  };
}
