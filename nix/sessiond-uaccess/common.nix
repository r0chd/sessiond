{
  lib,
  pkgs,
  ...
}:
let
  inherit (lib) types;
in
{
  options.services.sessiond-uaccess = {
    enable = lib.mkOption {
      type = types.bool;
      default = false;
      description = ''
        Whether to enable [sessiond](${pkgs.sessiond-uaccess.meta.homepage}) as a system service.
      '';
    };

    package = lib.mkOption {
      type = types.package;
      default = pkgs.sessiond-uaccess;
      defaultText = lib.literalExpression "pkgs.sessiond-uaccess";
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

    extraConfig = lib.mkOption {
      type = types.lines;
      default = "";
      example = ''
        device {
          access = "rw",
          match = function(dev)
            return dev.subsystem == "backlight"
          end,
        }
      '';
      description = ''
        Additional `lua` rules loaded after the packaged `sessiond-uaccess` rules. See [upstream documentation](https://tangled.org/r0chd.pl/sessiond-uaccess/blob/master/rules/70-uaccess.lua)
        for additional details.
      '';
    };
  };
}
