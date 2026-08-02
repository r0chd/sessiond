{
  pkgs,
  sessiondTest,
  sessiondModule,
  sessiondPackage,
}:
let
  virtualTerminals = builtins.genList (index: "tty${toString (index + 1)}") 9;
in
sessiondTest {
  name = "sessiond-vt-switch-finix";

  nodes.machine = {
    imports = [ sessiondModule ];

    environment = {
      systemPackages = [
        pkgs.jq
        pkgs.kbd
      ];
    };

    programs.bash.enable = true;
    services.mdevd.enable = true;
    services.getty = {
      enable = true;
      package = pkgs.writeShellScriptBin "agetty" ''
        for arg in "$@"; do
          if [ "$arg" = tty1 ]; then
            exec ${pkgs.util-linux}/bin/agetty --autologin user_1 --noclear "$@"
          fi
          if [ "$arg" = tty2 ]; then
            while [ ! -e /run/sessiond-test-login-user-1-tty2 ]; do
              sleep 0.1
            done
            exec ${pkgs.util-linux}/bin/agetty --autologin user_1 --noclear "$@"
          fi
          if [ "$arg" = tty3 ]; then
            while [ ! -e /run/sessiond-test-login-user-2-tty3 ]; do
              sleep 0.1
            done
            exec ${pkgs.util-linux}/bin/agetty --autologin user_2 --noclear "$@"
          fi
        done

        exec ${pkgs.util-linux}/bin/agetty --noclear "$@"
      '';
      ttys = virtualTerminals;
    };
    services.sessiond = {
      enable = true;
      package = sessiondPackage;
    };
    virtualisation.qemu.extraArgs = [
      "-display"
      "none"
      "-device"
      "virtio-vga"
      "-global"
      "isa-fdc.fdtypeA=none"
      "-global"
      "isa-fdc.fdtypeB=none"
    ];

    users.users = {
      user_1 = {
        isNormalUser = true;
        password = "";
        shell = pkgs.bashInteractive;
        createHome = true;
        extraGroups = [ "wheel" ];
      };
      user_2 = {
        isNormalUser = true;
        password = "";
        createHome = true;
        extraGroups = [ "wheel" ];
      };
    };
  };

  testScript = ''
    os.environ["DISPLAY"] = ":0"
    machine.start()

    machine.wait_for_console_text("finix - stage 1")
    machine.wait_for_console_text("finix - stage 2")
    machine.wait_for_console_text("entering runlevel 2")

    machine.wait_until_succeeds("test -S /run/sessiond.sock")

    machine.succeed("chvt 1")
    machine.wait_until_succeeds("fgconsole | grep -qx 1")
    machine.wait_until_succeeds("sessionctl session list --json > /tmp/sessions.json && jq -e '.sessions | length == 1' /tmp/sessions.json >/dev/null && jq -e '.sessions[] | select(.username == \"user_1\" and .vt == 1 and .tty == \"tty1\" and .active == \"active\")' /tmp/sessions.json >/dev/null")

    machine.succeed("touch /run/sessiond-test-login-user-1-tty2")
    machine.wait_until_succeeds("sessionctl session list --json > /tmp/sessions.json && jq -e '.sessions | length == 2' /tmp/sessions.json >/dev/null")
    machine.succeed("chvt 2")
    machine.wait_until_succeeds("fgconsole | grep -qx 2")
    machine.wait_until_succeeds("sessionctl session list --json > /tmp/sessions.json && jq -e '.sessions | length == 2' /tmp/sessions.json >/dev/null && jq -e '[.sessions[] | select(.active == \"active\")] | length == 1' /tmp/sessions.json >/dev/null && jq -e '.sessions[] | select(.username == \"user_1\" and .vt == 1 and .tty == \"tty1\" and .active == \"online\")' /tmp/sessions.json >/dev/null && jq -e '.sessions[] | select(.username == \"user_1\" and .vt == 2 and .tty == \"tty2\" and .active == \"active\")' /tmp/sessions.json >/dev/null")

    machine.succeed("touch /run/sessiond-test-login-user-2-tty3")
    machine.wait_until_succeeds("sessionctl session list --json > /tmp/sessions.json && jq -e '.sessions | length == 3' /tmp/sessions.json >/dev/null")
    machine.succeed("chvt 3")
    machine.wait_until_succeeds("fgconsole | grep -qx 3")
    machine.wait_until_succeeds("sessionctl session list --json > /tmp/sessions.json && jq -e '.sessions | length == 3' /tmp/sessions.json >/dev/null && jq -e '[.sessions[] | select(.active == \"active\")] | length == 1' /tmp/sessions.json >/dev/null && jq -e '.sessions[] | select(.username == \"user_1\" and .vt == 1 and .tty == \"tty1\" and .active == \"online\")' /tmp/sessions.json >/dev/null && jq -e '.sessions[] | select(.username == \"user_1\" and .vt == 2 and .tty == \"tty2\" and .active == \"online\")' /tmp/sessions.json >/dev/null && jq -e '.sessions[] | select(.username == \"user_2\" and .vt == 3 and .tty == \"tty3\" and .active == \"active\")' /tmp/sessions.json >/dev/null")

    machine.shutdown()
  '';
}
