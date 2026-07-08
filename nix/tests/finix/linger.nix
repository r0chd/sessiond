{
  pkgs,
  sessiondTest,
  sessiondModule,
  sessiondPackage,
}:
sessiondTest {
  name = "sessiond-basic-finix";

  nodes.machine = {
    imports = [ sessiondModule ];

    environment = {
      systemPackages = [ pkgs.jq ];
      etc."profile.d/sessiond-test-login-ready.sh".text = ''
        case "''${USER:-}" in
          user_1|user_2) echo "__SESSIOND_TEST_LOGIN_READY__:''${USER}" ;;
        esac
      '';
    };

    programs.bash.enable = true;
    services.mdevd.enable = true;
    services.getty = {
      enable = true;
      package = pkgs.writeShellScriptBin "agetty" ''
        exec ${pkgs.util-linux}/bin/agetty "$@"
      '';
      extraArgs = [
        "--autologin"
        "user_1"
        "--noclear"
      ];
      ttys = [ "ttyS0" ];
    };
    services.sessiond = {
      enable = true;
      package = sessiondPackage;
      settings.users = {
        user_2.linger = true;
        user_3.linger = true;
      };
    };

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
      user_3 = {
        isNormalUser = true;
        password = "";
        createHome = true;
        extraGroups = [ "wheel" ];
      };
    };
  };

  testScript = ''
    machine.start()

    machine.wait_for_console_text("finix - stage 1")
    machine.wait_for_console_text("finix - stage 2")
    machine.wait_for_console_text("entering runlevel 2")

    machine.wait_until_succeeds("test -S /run/sessiond.sock")
    machine.succeed("test -x ${sessiondPackage}/bin/sessiond")
    machine.succeed("grep -q pam_sessiond.so /etc/pam.d/login")

    machine.wait_for_console_text("__SESSIOND_TEST_LOGIN_READY__:user_1")
    machine.succeed("sessionctl session list --json > /tmp/sessions.json")
    machine.succeed("jq -e '.sessions | length == 1' /tmp/sessions.json >/dev/null")
    machine.succeed("jq -e '.sessions[0].active == \"online\"' /tmp/sessions.json >/dev/null")
    machine.succeed("jq -e '.sessions[0].id == 1' /tmp/sessions.json >/dev/null")
    machine.succeed("jq -e '.sessions[0].seat == \"seat0\"' /tmp/sessions.json >/dev/null")
    machine.succeed("jq -e '.sessions[0].uid == 1000' /tmp/sessions.json >/dev/null")
    machine.succeed("jq -e '.sessions[0].username == \"user_1\"' /tmp/sessions.json >/dev/null")
    machine.succeed("jq -e '.sessions[0].vt == 0' /tmp/sessions.json >/dev/null")
    machine.succeed("jq -e '.sessions[0].tty == \"tty0\"' /tmp/sessions.json >/dev/null")
    machine.succeed("jq -e '.sessions[0].leader > 0' /tmp/sessions.json >/dev/null")
    machine.succeed("jq -e '.sessions[0].timestamp > 0' /tmp/sessions.json >/dev/null")

    user_2_uid = machine.succeed("id -u user_2").strip()
    user_3_uid = machine.succeed("id -u user_3").strip()

    for uid in (user_2_uid, user_3_uid):
        machine.wait_until_succeeds(f"test -f /var/lib/sessiond/{uid}")
        machine.wait_until_succeeds(f"test -d /run/user/{uid}")
        machine.wait_until_succeeds(f"test -d /sys/fs/cgroup/sessiond/user-{uid}")

    machine.succeed(f"sessionctl user linger {user_2_uid} off")
    machine.fail(f"test -f /var/lib/sessiond/{user_2_uid}")
    machine.fail(f"test -d /run/user/{user_2_uid}")
    machine.fail(f"test -d /sys/fs/cgroup/sessiond/user-{user_2_uid}")

    machine.succeed(f"test -f /var/lib/sessiond/{user_3_uid}")
    machine.succeed(f"test -d /run/user/{user_3_uid}")
    machine.succeed(f"test -d /sys/fs/cgroup/sessiond/user-{user_3_uid}")

    machine.shutdown()
  '';
}
