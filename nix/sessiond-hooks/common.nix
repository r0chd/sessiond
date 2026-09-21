{
  lib,
  pkgs,
  ...
}:
let
  inherit (lib) types;
  tomlFormat = pkgs.formats.toml { };
in
{
  options.services.sessiond-hooks = {
    package = lib.mkPackageOption pkgs "sessiond-hooks" { };
    debug = lib.mkOption {
      type = types.bool;
      default = false;
      description = ''
        Whether to enable debug logging.
      '';
    };
    hooks = lib.mkOption {
      type = types.attrsOf (
        types.submodule {
          options = {
            enable = lib.mkOption {
              type = types.bool;
              default = true;
            };
            settings = lib.mkOption {
              type = tomlFormat.type;
              description = "Hook settings serialized to TOML.";
            };
          };
        }
      );
      default = { };
      description = ''
        Per-user session hooks launched. Each named entry spawns an
        independent `sessiond-hooks` process.      '';
    };
  };
}
