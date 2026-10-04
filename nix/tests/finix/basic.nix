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
  name = "sessiond-basic-finix";

  nodes.machine = {
    imports = [ sessiondModule ];

    environment = {
      systemPackages = [
        pkgs.jq
        pkgs.kbd
      ];
      etc."profile.d/sessiond-test-login-ready.sh".text = ''
        case "''${USER:-}" in
          user_1|user_2)
            echo "__SESSIOND_TEST_LOGIN_READY__:''${USER}"
            echo "__SESSIOND_TEST_ENV__:''${USER}:''${XDG_RUNTIME_DIR:-}:''${TEST:-}:''${DBUS_SESSION_BUS_ADDRESS:-}:''${XDG_SESSION_ID:-}"
            ;;
        esac
      '';
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
        done

        exec ${pkgs.util-linux}/bin/agetty --noclear "$@"
      '';
      ttys = virtualTerminals;
    };
    services.sysklogd.enable = true;
    services.sessiond = {
      enable = true;
      package = sessiondPackage;
      settings = {
        users.user_1 = {
          runtime-dir = "/tmp/{uid}";
          env = {
            TEST = "set";
          };
        };
      };
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
    };
  };

  testScript = ''
    os.environ["DISPLAY"] = ":0"
    machine.start()

    machine.wait_for_console_text("finix - stage 1")
    machine.wait_for_console_text("finix - stage 2")
    machine.wait_for_console_text("entering runlevel 2")

    machine.wait_until_succeeds("test -S /run/sessiond/sessiond.sock")

    machine.succeed("chvt 1")
    machine.wait_until_succeeds("fgconsole | grep -qx 1")
    machine.wait_until_succeeds("sessionctl session list --json > /tmp/sessions.json && jq -e '.sessions | length == 1' /tmp/sessions.json >/dev/null")
    machine.wait_until_succeeds("test -d /tmp/1000")
    machine.succeed("test \"$(stat -c %U:%G:%a /tmp/1000)\" = user_1:root:700")
    machine.succeed("jq -e '[.sessions[] | select(.state == \"active\")] | length == 1' /tmp/sessions.json >/dev/null")
    machine.succeed("jq -e '.sessions[] | select(.state == \"active\" and .vt == 1 and .tty == \"tty1\")' /tmp/sessions.json >/dev/null")
    machine.succeed("jq -e 'all(.sessions[]; .uid == 1000 and .username == \"user_1\" and .leader > 0 and .timestamp > 0)' /tmp/sessions.json >/dev/null")

    machine.succeed("test -d /sys/fs/cgroup/system/sessiond/user-1000")
    machine.succeed("test -d /sys/fs/cgroup/system/sessiond/user-1000/session-1")

    machine.shutdown()
  '';
}
