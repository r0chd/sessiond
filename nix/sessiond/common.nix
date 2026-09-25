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

    runtimeDir = lib.mkOption {
      type = types.strMatching "/.*";
      default = "/run/sessiond";
      description = ''
        Absolute path to the sessiond runtime directory containing its socket and state.
        Passed to companion services as `SESSIOND_RUNTIME_DIR`.
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
