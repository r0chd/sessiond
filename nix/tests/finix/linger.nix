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
  name = "sessiond-linger-finix";

  nodes.machine = {
    imports = [ sessiondModule ];

    environment = {
      systemPackages = [
        pkgs.jq
        pkgs.kbd
      ];
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
        for arg in "$@"; do
          if [ "$arg" = tty1 ]; then
            exec ${pkgs.util-linux}/bin/agetty --autologin user_1 --noclear "$@"
          fi
          if [ "$arg" = tty2 ]; then
            while [ ! -e /run/sessiond-test-login-user-2-tty2 ]; do
              sleep 0.1
            done
            exec ${pkgs.util-linux}/bin/agetty --autologin user_2 --noclear "$@"
          fi
          if [ "$arg" = tty3 ]; then
            while [ ! -e /run/sessiond-test-login-user-2-tty3 ]; do
              sleep 0.1
            done
            exec ${pkgs.util-linux}/bin/agetty --autologin user_2 --noclear "$@"
          fi
          if [ "$arg" = tty4 ]; then
            while [ ! -e /run/sessiond-test-login-user-2-tty4 ]; do
              sleep 0.1
            done
            exec ${pkgs.util-linux}/bin/agetty --autologin user_2 --noclear "$@"
          fi
          if [ "$arg" = tty5 ]; then
            while [ ! -e /run/sessiond-test-login-user-2-tty5 ]; do
              sleep 0.1
            done
            exec ${pkgs.util-linux}/bin/agetty --autologin user_2 --noclear "$@"
          fi
          if [ "$arg" = tty6 ]; then
            while [ ! -e /run/sessiond-test-login-user-3-tty6 ]; do
              sleep 0.1
            done
            exec ${pkgs.util-linux}/bin/agetty --autologin user_3 --noclear "$@"
          fi
          if [ "$arg" = tty7 ]; then
            while [ ! -e /run/sessiond-test-login-user-3-tty7 ]; do
              sleep 0.1
            done
            exec ${pkgs.util-linux}/bin/agetty --autologin user_3 --noclear "$@"
          fi
        done

        exec ${pkgs.util-linux}/bin/agetty --noclear "$@"
      '';
      ttys = virtualTerminals;
    };
    services = {
      sysklogd.enable = true;
      sessiond = {
        enable = true;
        package = sessiondPackage;
        settings.users = {
          user_2.linger = true;
          user_3.linger = true;
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
    os.environ["DISPLAY"] = ":0"
    machine.start()

    machine.wait_for_console_text("finix - stage 1")
    machine.wait_for_console_text("finix - stage 2")
    machine.wait_for_console_text("entering runlevel 2")

    machine.wait_until_succeeds("test -S /run/sessiond/sessiond.sock")
    machine.succeed("test -x ${sessiondPackage}/bin/sessiond")
    machine.succeed("pam_path=$(awk '/pam_sessiond.so/ {print $3}' /etc/pam.d/login); test -e \"$pam_path\"")

    machine.succeed("chvt 1")
    machine.wait_until_succeeds("fgconsole | grep -qx 1")
    machine.wait_until_succeeds("sessionctl session list --json > /tmp/sessions.json && jq -e '.sessions | length >= 1' /tmp/sessions.json >/dev/null")
    machine.succeed("jq -e '[.sessions[] | select(.active == \"active\")] | length == 1' /tmp/sessions.json >/dev/null")
    machine.succeed("jq -e '.sessions[] | select(.active == \"active\" and .vt == 1 and .tty == \"tty1\")' /tmp/sessions.json >/dev/null")
    machine.succeed("jq -e 'all(.sessions[]; .uid == 1000 and .username == \"user_1\" and .leader > 0 and .timestamp > 0)' /tmp/sessions.json >/dev/null")

    user_2_uid = machine.succeed("id -u user_2").strip()
    user_3_uid = machine.succeed("id -u user_3").strip()

    machine.wait_until_succeeds("sessionctl user list --json > /tmp/users.json && jq -e '.users | length == 3' /tmp/users.json >/dev/null")
    machine.succeed(f"jq -e '.users[] | select(.uid == {user_2_uid} and .username == \"user_2\" and .state == \"lingering\" and .lingering)' /tmp/users.json >/dev/null")
    machine.succeed(f"jq -e '.users[] | select(.uid == {user_3_uid} and .username == \"user_3\" and .state == \"lingering\" and .lingering)' /tmp/users.json >/dev/null")

    for uid in (user_2_uid, user_3_uid):
        machine.wait_until_succeeds(f"test -d /run/user/{uid}")
        machine.wait_until_succeeds(f"test -d /sys/fs/cgroup/system/sessiond/user-{uid}")

    def wait_session(username, vt):
        machine.wait_until_succeeds(f"sessionctl session list --json > /tmp/sessions.json && jq -e '.sessions[] | select(.username == \"{username}\" and .vt == {vt})' /tmp/sessions.json >/dev/null")

    def wait_no_sessions(username):
        machine.wait_until_succeeds(f"sessionctl session list --json > /tmp/sessions.json && jq -e 'all(.sessions[]; .username != \"{username}\")' /tmp/sessions.json >/dev/null")

    def session_id(username, vt):
        return machine.succeed(f"sessionctl session list --json | jq -r '.sessions[] | select(.username == \"{username}\" and .vt == {vt}) | .id'").strip()

    def session_cgroup(uid, session_id):
        return f"/sys/fs/cgroup/system/sessiond/user-{uid}/session-{session_id}"

    def assert_linger_paths(uid):
        machine.succeed(f"test -d /run/user/{uid}")
        machine.succeed(f"test -d /sys/fs/cgroup/system/sessiond/user-{uid}")

    def assert_session_cgroup(uid, session_id):
        machine.succeed(f"test -d {session_cgroup(uid, session_id)}")

    def wait_no_session_cgroup(uid, session_id):
        machine.wait_until_succeeds(f"test ! -e {session_cgroup(uid, session_id)}")

    def login(marker, username, vt):
        machine.succeed(f"touch /run/{marker}")
        wait_session(username, vt)

    login("sessiond-test-login-user-2-tty2", "user_2", 2)
    session = session_id("user_2", 2)
    assert_session_cgroup(user_2_uid, session)
    machine.succeed("rm -f /run/sessiond-test-login-user-2-tty2")
    machine.succeed(f"sessionctl session close {session}")
    wait_no_sessions("user_2")
    wait_no_session_cgroup(user_2_uid, session)
    assert_linger_paths(user_2_uid)
    machine.wait_until_succeeds(f"sessionctl user list --json | jq -e '.users[] | select(.uid == {user_2_uid} and .state == \"lingering\" and .lingering)' >/dev/null")

    login("sessiond-test-login-user-2-tty3", "user_2", 3)
    session = session_id("user_2", 3)
    assert_session_cgroup(user_2_uid, session)
    machine.succeed("rm -f /run/sessiond-test-login-user-2-tty3")
    machine.succeed(f"sessionctl session kill {session}")
    wait_no_sessions("user_2")
    wait_no_session_cgroup(user_2_uid, session)
    assert_linger_paths(user_2_uid)

    login("sessiond-test-login-user-2-tty4", "user_2", 4)
    login("sessiond-test-login-user-2-tty5", "user_2", 5)
    user_2_tty4 = session_id("user_2", 4)
    user_2_tty5 = session_id("user_2", 5)
    assert_session_cgroup(user_2_uid, user_2_tty4)
    assert_session_cgroup(user_2_uid, user_2_tty5)
    machine.succeed("rm -f /run/sessiond-test-login-user-2-tty4 /run/sessiond-test-login-user-2-tty5")
    machine.succeed(f"sessionctl user terminate {user_2_uid}")
    wait_no_sessions("user_2")
    wait_no_session_cgroup(user_2_uid, user_2_tty4)
    wait_no_session_cgroup(user_2_uid, user_2_tty5)
    assert_linger_paths(user_2_uid)

    login("sessiond-test-login-user-3-tty6", "user_3", 6)
    login("sessiond-test-login-user-3-tty7", "user_3", 7)
    user_3_tty6 = session_id("user_3", 6)
    user_3_tty7 = session_id("user_3", 7)
    assert_session_cgroup(user_3_uid, user_3_tty6)
    assert_session_cgroup(user_3_uid, user_3_tty7)
    machine.succeed("rm -f /run/sessiond-test-login-user-3-tty6 /run/sessiond-test-login-user-3-tty7")
    machine.succeed(f"sessionctl user kill {user_3_uid}")
    wait_no_sessions("user_3")
    wait_no_session_cgroup(user_3_uid, user_3_tty6)
    wait_no_session_cgroup(user_3_uid, user_3_tty7)
    assert_linger_paths(user_3_uid)

    machine.shutdown()
  '';
}
