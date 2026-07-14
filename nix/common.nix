{
  lib,
  pkgs,
  config,
  ...
}:
let
  inherit (lib) types;
  cfg = config.services.sessiond;
  settingsFormat = pkgs.formats.toml { };
  seatGroups = lib.unique (
    lib.filter (group: group != null) (
      lib.mapAttrsToList (_: seat: seat.group or null) (cfg.settings.seats or { })
    )
  );
in
{
  options.services.sessiond = {
    enable = lib.mkEnableOption "sessiond";
    package = lib.mkPackageOption pkgs "sessiond" { };
    logLevel = lib.mkOption {
      type = types.enum [
        "trace"
        "debug"
        "info"
        "warn"
        "error"
        "off"
      ];
      default = "info";
      description = "Maximum verbosity of sessiond log messages.";
    };
    settings = lib.mkOption {
      type = settingsFormat.type;
      default = { };
      description = "sessiond daemon configuration, serialized directly to sessiond.toml.";
    };
  };

  config = lib.mkIf cfg.enable {
    services.sessiond.settings.default-seat = lib.mkDefault "seat0";
    services.sessiond.settings.seats.seat0 = {
      user = lib.mkDefault "root";
      group = lib.mkDefault "seat";
    };

    environment.systemPackages = [ cfg.package ];
    services.dbus.packages = [ cfg.package ];

    users.groups = lib.genAttrs seatGroups (_: { });

    security.pam.services.login.sessiond = lib.mkDefault true;
  };
}
