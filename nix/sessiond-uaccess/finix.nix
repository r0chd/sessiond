{
  lib,
  pkgs,
  config,
  ...
}:
let
  inherit (lib) types;
  cfg = config.services.sessiond-uaccess;
  extraRuleNames = builtins.attrNames cfg.extraRules;
  validRuleName = name: builtins.match "[0-9A-Za-z._+-]+[.]lua" name != null;
  extraRulesPackage = pkgs.linkFarm "sessiond-uaccess-extra-rules" (
    lib.mapAttrsToList (name: text: {
      inherit name;
      path = pkgs.writeText name text;
    }) cfg.extraRules
  );
in
{
  options.services.sessiond-uaccess = {
    enable = lib.mkEnableOption "sessiond-uaccess";
    package = lib.mkPackageOption pkgs "sessiond-uaccess" { };
    debug = lib.mkOption {
      type = types.bool;
      default = false;
      description = ''
        Whether to enable debug logging.
      '';
    };
    extraRules = lib.mkOption {
      type = types.attrsOf types.lines;
      default = { };
      example = {
        "90-local.lua" = ''
          device {
            access = "rw",
            match = function(dev)
              return dev.subsystem == "backlight"
            end,
          }
        '';
      };
      description = ''
        Additional Lua rule files loaded after the packaged sessiond-uaccess
        rules. Attribute names are used as filenames and should end in
        {file}`.lua`; numeric prefixes can be used to express ordering.
      '';
    };
  };

  config = lib.mkIf cfg.enable {
    assertions = [
      {
        assertion = builtins.all validRuleName extraRuleNames;
        message = "services.sessiond-uaccess.extraRules attribute names must be simple Lua filenames ending in .lua.";
      }
    ];

    finit.services.sessiond-uaccess = {
      description = "Grant device access to active local sessions";
      conditions = [
        "service/sessiond/ready"
        "service/syslogd/ready"
      ];
      command = lib.escapeShellArgs (
        [
          (lib.getExe cfg.package)
          "--log-target"
          "syslog"
          "--rules-dirs"
          "${cfg.package}/share/sessiond-uaccess/rules"
        ]
        ++ lib.optionals (cfg.extraRules != { }) [
          "--rules-dirs"
          "${extraRulesPackage}"
        ]
      );
      environment =
        if cfg.debug then
          {
            LOG_LEVEL = "debug";
          }
        else
          {
            LOG_LEVEL = lib.mkDefault "info";
          };
    };
  };
}
