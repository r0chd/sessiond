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
    settings = {
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
      users = lib.mkOption {
        type = lib.types.attrsOf (
          lib.types.submodule (
            { ... }: {
              options = {
                linger = lib.mkOption {
                  type = types.bool;
                  default = false;
                  description = "Whether to keep this user's sessiond user state alive without active sessions.";
                };
              };
            }
          )
        );
        default = { };
      };
    };
  };

  config = lib.mkIf cfg.enable {
    environment.systemPackages = [ cfg.package ];
    services.dbus.packages = [ cfg.package ];

    users.groups.${cfg.settings.seat.group} = { };

    security.pam.services.login.sessiond = lib.mkDefault true;
  };
}
