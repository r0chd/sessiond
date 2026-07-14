{
  lib,
  pkgs,
  config,
  ...
}:
let
  cfg = config.services.sessiond;
  lingerUsersScript = import ./linger-users.nix { inherit lib pkgs cfg; };
  settingsFormat = pkgs.formats.toml { };
  daemonConfig = settingsFormat.generate "sessiond.toml" {
    seats.seat0 = {
      inherit (cfg.settings.seat) user group;
    };
    users = lib.mapAttrs (_: user: {
      inherit (user) linger;
      cgroup = { };
    }) cfg.settings.users;
  };
in
{
  imports = [ ./common.nix ];

  options.security.pam.services = lib.mkOption {
    type = lib.types.attrsOf (
      lib.types.submodule (
        { config, ... }:
        {
          options.sessiond = lib.mkEnableOption "opening sessiond sessions";

          config.text = lib.mkIf config.sessiond (
            lib.mkAfter ''
              session   optional   ${cfg.package}/lib/security/pam_sessiond.so
            ''
          );
        }
      )
    );
  };

  config = lib.mkIf cfg.enable {
    finit = {
      services.sessiond = {
        description = "session management daemon";
        conditions = [ ];
        command = "${cfg.package}/bin/sessiond --config ${daemonConfig}";
        environment.LOG_LEVEL = cfg.logLevel;
        log = true;
      };
      tasks.sessiond-linger-users = lib.mkIf (cfg.settings.users != { }) {
        description = "sessiond linger users";
        conditions = [ ];
        command = "${lingerUsersScript}";
        log = true;
      };
    };

    services.polkit.package = pkgs.callPackage ./polkit {
      useConsoleKit = true;
      useSystemd = false;
    };
  };
}
