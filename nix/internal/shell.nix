{
  pkgs,
  rustToolchain,
  sessiond,
}:
pkgs.mkShell {
  name = "sessiond-devshell";
  packages = builtins.attrValues {
    sessiond-testctl = pkgs.writeShellScriptBin "sessiond-testctl" ''
      printf 'sessiond-testctl args:'
      printf ' <%s>' "$@"
      printf '\n'
    '';
    inherit
      rustToolchain
      ;
    inherit (pkgs)
      rust-analyzer-unwrapped
      nixd
      npins
      linux-pam
      pkg-config
      ;
  };

  nativeBuildInputs = [ pkgs.pkg-config ];
  buildInputs = [ sessiond ];

  RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";

  shellHook = ''
    if [[ $- == *i* ]] && [ -z "''${IN_NIX_SHELL_NU:-}" ]; then
      export IN_NIX_SHELL_NU=1
      exec ${pkgs.nushell}/bin/nu
    fi
  '';
}
