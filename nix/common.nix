{
  lib,
  pkgs,
  ...
}:
let
  inherit (lib) types;
in
{
  options.services.sessiond = {
    enable = lib.mkEnableOption "sessiond";
    package = lib.mkPackageOption pkgs "sessiond" { };
    seat = {
      user = lib.mkOption {
        type = types.str;
        default = "root";
        description = "User to own the seatd socket";
      };
      group = lib.mkOption {
        type = types.str;
        default = "seat";
        description = "Group to own the seatd socket";
      };
    };
  };
}
