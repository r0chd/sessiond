{
  lib,
  pkgs,
  config,
  ...
}:
let
  cfg = config.services.sessiond;
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

  config = lib.mkIf cfg.enable {
    finit.services.sessiond = {
      description = "session management daemon";
      conditions = [ ];
      command = "${cfg.package}/bin/sessiond --seat-user ${cfg.seat.user} --seat-group ${cfg.seat.group}";
      log = true;
    };

    security.pam.services.login.text = lib.mkAfter ''
      session   optional   ${cfg.package}/lib/security/pam_sessiond.so
    '';

    environment.systemPackages = [ cfg.package ];

    users.groups.${cfg.seat.group} = { };
  };
}
