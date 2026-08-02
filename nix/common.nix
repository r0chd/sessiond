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
  normalUsers = lib.filterAttrs (_: user: user.isNormalUser or false) config.users.users;
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
    services.sessiond.settings = {
      users = lib.mapAttrs (_userName: _userCfg: {
        env.DBUS_SESSION_BUS_ADDRESS = lib.mkDefault "unix:path={runtime-dir}/bus";
      }) normalUsers;
    };

    environment.systemPackages = [ cfg.package ];
    services.dbus.packages = [ cfg.package ];

  };
}
