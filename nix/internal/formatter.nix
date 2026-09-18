{ pkgs }:
pkgs.writeShellApplication {
  name = "nix3-fmt-wrapper";
  runtimeInputs = builtins.attrValues {
    inherit (pkgs)
      cargo
      rustfmt
      nixfmt
      taplo
      fd
      ;
  };
  text = ''
    fd "$@" -t f -e nix -x nixfmt -q '{}'
    fd "$@" -t f -e toml -x taplo format '{}'
    cargo fmt
  '';
}
