{
  lib,
  pkgs,
  config,
  ...
}:
let
  inherit (lib) types;
  cfg = config.services.sessiond;
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

  config = lib.mkIf cfg.enable {
    environment.systemPackages = [ cfg.package ];
    services.dbus.packages = [ cfg.package ];

    users.groups.${cfg.seat.group} = { };

    security.pam.services.login.sessiond = lib.mkDefault true;
  };
}
