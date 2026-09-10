{
  lib,
  pkgs,
  ...
}:
let
  format = pkgs.formats.toml { };
  inherit (lib) types;
in
{
  options.services.sessiond = {
    enable = lib.mkOption {
      type = types.bool;
      default = false;
      description = ''
        Whether to enable [sessiond](${pkgs.sessiond.meta.homepage}) as a system service.
      '';
    };

    package = lib.mkOption {
      type = types.package;
      default = pkgs.sessiond;
      defaultText = lib.literalExpression "pkgs.sessiond";
      description = ''
        The package to use for `sessiond`.
      '';
    };

    debug = lib.mkOption {
      type = types.bool;
      default = false;
      description = ''
        Whether to enable debug logging.
      '';
    };

    settings = lib.mkOption {
      type = format.type;
      default = { };
      description = ''
        `sessiond` configuration. See [upstream documentation](https://tangled.org/r0chd.pl/sessiond/blob/master/docs/src/README.md)
        for additional details.
      '';
    };
  };
}
