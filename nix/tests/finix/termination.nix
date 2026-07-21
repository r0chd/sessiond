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
  name = "sessiond-termination-finix";

  nodes.machine = {
    imports = [ sessiondModule ];

    environment.systemPackages = [
      pkgs.jq
      pkgs.kbd
    ];

    programs.bash.enable = true;
    services.mdevd.enable = true;
    services.getty = {
      enable = true;
      package = pkgs.writeShellScriptBin "agetty" ''
        case "$*" in
          *tty1*) user=user_1; marker=/run/sessiond-test-login-user-1-tty1 ;;
          *tty2*) user=user_2; marker=/run/sessiond-test-login-user-2-tty2 ;;
          *tty3*) user=user_2; marker=/run/sessiond-test-login-user-2-tty3 ;;
          *tty4*) user=user_1; marker=/run/sessiond-test-login-user-1-tty4 ;;
          *tty5*) user=user_1; marker=/run/sessiond-test-login-user-1-tty5 ;;
          *tty6*) user=user_2; marker=/run/sessiond-test-login-user-2-tty6 ;;
          *tty7*) user=user_2; marker=/run/sessiond-test-login-user-2-tty7 ;;
          *) exec ${pkgs.util-linux}/bin/agetty --noclear "$@" ;;
        esac

        while [ ! -e "$marker" ]; do
          sleep 0.1
        done
        exec ${pkgs.util-linux}/bin/agetty --autologin "$user" --noclear "$@"
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

    def wait_session_count(count):
        machine.wait_until_succeeds(f"sessionctl session list --json > /tmp/sessions.json && jq -e '.sessions | length == {count}' /tmp/sessions.json >/dev/null")

    def wait_session(username, vt):
        machine.wait_until_succeeds(f"sessionctl session list --json > /tmp/sessions.json && jq -e '.sessions[] | select(.username == \"{username}\" and .vt == {vt})' /tmp/sessions.json >/dev/null")

    def wait_no_session(username, vt):
        machine.wait_until_succeeds(f"sessionctl session list --json > /tmp/sessions.json && jq -e 'all(.sessions[]; .username != \"{username}\" or .vt != {vt})' /tmp/sessions.json >/dev/null")

    def session_id(username, vt):
        return machine.succeed(f"sessionctl session list --json | jq -r '.sessions[] | select(.username == \"{username}\" and .vt == {vt}) | .id'").strip()

    def cgroup_path(uid, session_id):
        return f"/sys/fs/cgroup/sessiond/user-{uid}/session-{session_id}"

    def assert_cgroup(uid, session_id):
        machine.succeed(f"test -d {cgroup_path(uid, session_id)}")

    def wait_no_cgroup(uid, session_id):
        machine.wait_until_succeeds(f"test ! -e {cgroup_path(uid, session_id)}")

    def wait_no_user_cgroup(uid):
        machine.wait_until_succeeds(f"test ! -e /sys/fs/cgroup/sessiond/user-{uid}")

    def login(marker, username, vt, count):
        machine.succeed(f"touch /run/{marker}")
        wait_session(username, vt)
        wait_session_count(count)

    login("sessiond-test-login-user-1-tty1", "user_1", 1, 1)
    user_1_tty1 = session_id("user_1", 1)
    assert_cgroup(1000, user_1_tty1)

    login("sessiond-test-login-user-2-tty2", "user_2", 2, 2)
    target = session_id("user_2", 2)
    assert_cgroup(1001, target)
    machine.succeed("rm -f /run/sessiond-test-login-user-2-tty2")
    machine.succeed(f"sessionctl session close {target}")
    wait_no_session("user_2", 2)
    wait_no_cgroup(1001, target)
    wait_no_user_cgroup(1001)
    wait_session_count(1)

    login("sessiond-test-login-user-2-tty3", "user_2", 3, 2)
    target = session_id("user_2", 3)
    assert_cgroup(1001, target)
    machine.succeed("rm -f /run/sessiond-test-login-user-2-tty3")
    machine.succeed(f"sessionctl session kill {target}")
    wait_no_session("user_2", 3)
    wait_no_cgroup(1001, target)
    wait_no_user_cgroup(1001)
    wait_session_count(1)

    login("sessiond-test-login-user-1-tty4", "user_1", 4, 2)
    login("sessiond-test-login-user-1-tty5", "user_1", 5, 3)
    user_1_tty4 = session_id("user_1", 4)
    user_1_tty5 = session_id("user_1", 5)
    assert_cgroup(1000, user_1_tty1)
    assert_cgroup(1000, user_1_tty4)
    assert_cgroup(1000, user_1_tty5)
    machine.succeed("rm -f /run/sessiond-test-login-user-1-tty1 /run/sessiond-test-login-user-1-tty4 /run/sessiond-test-login-user-1-tty5")
    machine.succeed("sessionctl user terminate 1000")
    machine.wait_until_succeeds("sessionctl session list --json > /tmp/sessions.json && jq -e 'all(.sessions[]; .username != \"user_1\")' /tmp/sessions.json >/dev/null")
    wait_no_cgroup(1000, user_1_tty1)
    wait_no_cgroup(1000, user_1_tty4)
    wait_no_cgroup(1000, user_1_tty5)
    wait_no_user_cgroup(1000)
    wait_session_count(0)

    login("sessiond-test-login-user-2-tty6", "user_2", 6, 1)
    login("sessiond-test-login-user-2-tty7", "user_2", 7, 2)
    user_2_tty6 = session_id("user_2", 6)
    user_2_tty7 = session_id("user_2", 7)
    assert_cgroup(1001, user_2_tty6)
    assert_cgroup(1001, user_2_tty7)
    machine.succeed("rm -f /run/sessiond-test-login-user-2-tty6 /run/sessiond-test-login-user-2-tty7")
    machine.succeed("sessionctl user kill 1001")
    machine.wait_until_succeeds("sessionctl session list --json > /tmp/sessions.json && jq -e 'all(.sessions[]; .username != \"user_2\")' /tmp/sessions.json >/dev/null")
    wait_no_cgroup(1001, user_2_tty6)
    wait_no_cgroup(1001, user_2_tty7)
    wait_no_user_cgroup(1001)
    wait_session_count(0)

    machine.shutdown()
  '';
}
