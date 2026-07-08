{
  lib,
  pkgs,
  cfg,
}:
let
  inherit (lib) filterAttrs mapAttrsToList;

  lingeringUsers = filterAttrs (_: user: user.linger == true) cfg.settings.users;
  nonLingeringUsers = filterAttrs (_: user: user.linger == false) cfg.settings.users;
  lingeringUserNames = mapAttrsToList (name: _: name) lingeringUsers;
  nonLingeringUserNames = mapAttrsToList (name: _: name) nonLingeringUsers;
in
pkgs.writeShellScript "sessiond-linger-users" ''
  set -euo pipefail

  ${lib.strings.toShellVars { inherit lingeringUserNames nonLingeringUserNames; }}

  user_configured() {
      ${pkgs.coreutils}/bin/id "$1" >/dev/null 2>&1
  }

  uid_configured() {
      ${pkgs.glibc.bin}/bin/getent passwd "$1" >/dev/null 2>&1
  }

  set_linger() {
      local state="$1"
      shift

      local user uid
      for user in "$@"; do
          if ! user_configured "$user"; then
              echo "Skipping linger $state for missing user $user" >&2
              continue
          fi

          uid="$(${pkgs.coreutils}/bin/id -u "$user")"
          ${cfg.package}/bin/sessionctl user linger "$uid" "$state"
      done
  }

  for _ in {1..100}; do
      if [ -S /run/sessiond.sock ]; then
          break
      fi
      ${pkgs.coreutils}/bin/sleep 0.1
  done

  if [ ! -S /run/sessiond.sock ]; then
      echo "sessiond socket did not appear" >&2
      exit 1
  fi

  if [ -d /var/lib/sessiond ]; then
      cd /var/lib/sessiond

      shopt -s dotglob nullglob
      for uid in *; do
          if ! uid_configured "$uid"; then
              echo "Removing linger for missing uid $uid" >&2
              ${pkgs.coreutils}/bin/rm -- "$uid"
          fi
      done
  fi

  if (( ''${#nonLingeringUserNames[*]} > 0 )); then
      set_linger off "''${nonLingeringUserNames[@]}"
  fi

  if (( ''${#lingeringUserNames[*]} > 0 )); then
      set_linger on "''${lingeringUserNames[@]}"
  fi
''
