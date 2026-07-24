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
    assertions = lib.mkIf cfg.enable [
      {
        assertion = builtins.hasAttr cfg.settings.default-seat (cfg.settings.seats or { });
        message = "services.sessiond.settings.default-seat must name a configured seat.";
      }
    ];

    services.sessiond.settings = {
      default-seat = lib.mkDefault "seat0";
      seats.seat0 = {
        user = lib.mkDefault "root";
        group = lib.mkDefault "seat";
      };
      users = lib.mapAttrs (_userName: _userCfg: {
        env.DBUS_SESSION_BUS_ADDRESS = lib.mkDefault "unix:path={runtime-dir}/bus";
      }) config.users.users;
    };

    environment.systemPackages = [ cfg.package ];
    services.dbus.packages = [ cfg.package ];

    users.groups = lib.genAttrs seatGroups (_: { });
  };
}
