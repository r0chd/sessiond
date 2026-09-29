{
  pkgs,
  sessiondTest,
  sessiondModule,
  sessiondPackage,
}:
let
  polkitTestAction = pkgs.writeTextFile {
    name = "sessiond-polkit-test-action";
    destination = "/share/polkit-1/actions/org.sessiond.consolekit-test.policy";
    text = ''
      <?xml version="1.0" encoding="UTF-8"?>
      <!DOCTYPE policyconfig PUBLIC
        "-//freedesktop//DTD PolicyKit Policy Configuration 1.0//EN"
        "http://www.freedesktop.org/standards/PolicyKit/1/policyconfig.dtd">
      <policyconfig>
        <vendor>sessiond test</vendor>
        <action id="org.sessiond.consolekit-test">
          <description>Test ConsoleKit subject resolution</description>
          <message>Testing ConsoleKit subject resolution</message>
          <defaults>
            <allow_any>no</allow_any>
            <allow_inactive>no</allow_inactive>
            <allow_active>no</allow_active>
          </defaults>
        </action>
      </policyconfig>
    '';
  };
in
sessiondTest {
  name = "sessiond-consolekit-finix";

  nodes.machine = {
    imports = [ sessiondModule ];

    environment = {
      systemPackages = [
        pkgs.jq
        polkitTestAction
      ];
      etc."profile.d/sessiond-test-login-ready.sh".text = ''
        case "''${USER:-}" in
          user_1) echo "__SESSIOND_TEST_LOGIN_READY__:''${USER}" ;;
        esac
      '';
    };

    programs.bash.enable = true;
    services.dbus.enable = true;
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
    services.mdevd.enable = true;
    services.sysklogd.enable = true;
    services.polkit = {
      enable = true;
      extraConfig = ''
        polkit.addRule(function(action, subject) {
          if (
            action.id == "org.sessiond.consolekit-test" &&
            subject.local &&
            subject.active
          ) {
            return polkit.Result.YES;
          }

          return polkit.Result.NO;
        });
      '';
    };
    services.sessiond = {
      enable = true;
      package = sessiondPackage;
    };

    users.users.user_1 = {
      isNormalUser = true;
      password = "";
      shell = pkgs.bashInteractive;
      createHome = true;
    };
  };

  testScript = ''
    machine.start()

    machine.wait_for_console_text("finix - stage 1")
    machine.wait_for_console_text("finix - stage 2")
    machine.wait_for_console_text("entering runlevel 2")

    machine.wait_until_succeeds("test -S /run/sessiond/sessiond.sock")
    machine.wait_until_succeeds("test -S /run/dbus/system_bus_socket")
    machine.wait_for_console_text("__SESSIOND_TEST_LOGIN_READY__:user_1")
    machine.succeed("sessionctl session list --json > /tmp/sessions.json")
    machine.succeed("jq -e '.sessions | length == 1' /tmp/sessions.json >/dev/null")
    pid = machine.succeed("jq -r '.sessions[0].leader' /tmp/sessions.json").strip()
    uid = machine.succeed("jq -r '.sessions[0].uid' /tmp/sessions.json").strip()

    machine.wait_until_succeeds("${pkgs.systemd}/bin/busctl --system call org.freedesktop.DBus /org/freedesktop/DBus org.freedesktop.DBus NameHasOwner s org.freedesktop.ConsoleKit | grep -q true")
    machine.wait_until_succeeds("${pkgs.systemd}/bin/busctl --system call org.freedesktop.DBus /org/freedesktop/DBus org.freedesktop.DBus NameHasOwner s org.freedesktop.PolicyKit1 | grep -q true")
    machine.succeed("${pkgs.systemd}/bin/busctl --system call org.freedesktop.ConsoleKit /org/freedesktop/ConsoleKit/Manager org.freedesktop.ConsoleKit.Manager GetSessions")
    machine.succeed(f"${pkgs.systemd}/bin/busctl --system call org.freedesktop.ConsoleKit /org/freedesktop/ConsoleKit/Manager org.freedesktop.ConsoleKit.Manager GetSessionForUnixProcess u {pid} > /tmp/ck-session")
    machine.succeed("grep -q '/org/freedesktop/ConsoleKit/Session/' /tmp/ck-session")
    machine.succeed("${pkgs.systemd}/bin/busctl --system introspect org.freedesktop.ConsoleKit /org/freedesktop/ConsoleKit/Manager org.freedesktop.ConsoleKit.Manager")

    session = machine.succeed(f"${pkgs.systemd}/bin/busctl --system call org.freedesktop.ConsoleKit /org/freedesktop/ConsoleKit/Manager org.freedesktop.ConsoleKit.Manager GetSessionForUnixProcess u {pid} | cut -d '\"' -f 2").strip()
    machine.succeed("test -f /var/run/ConsoleKit/database")
    machine.succeed(f"grep -F '[Session {session}]' /var/run/ConsoleKit/database")
    machine.succeed(f"grep -F 'uid={uid}' /var/run/ConsoleKit/database")
    machine.succeed("grep -F 'is_active=true' /var/run/ConsoleKit/database")
    machine.succeed("grep -F 'is_local=true' /var/run/ConsoleKit/database")
    machine.succeed(f"${pkgs.systemd}/bin/busctl --system introspect org.freedesktop.ConsoleKit {session} org.freedesktop.ConsoleKit.Session")
    machine.succeed(f"${pkgs.systemd}/bin/busctl --system call org.freedesktop.ConsoleKit {session} org.freedesktop.ConsoleKit.Session GetUnixUser")
    machine.succeed(f"${pkgs.systemd}/bin/busctl --system call org.freedesktop.ConsoleKit {session} org.freedesktop.ConsoleKit.Session GetUser")
    machine.succeed(f"${pkgs.systemd}/bin/busctl --system call org.freedesktop.ConsoleKit {session} org.freedesktop.ConsoleKit.Session IsActive")
    machine.succeed(f"${pkgs.systemd}/bin/busctl --system call org.freedesktop.ConsoleKit {session} org.freedesktop.ConsoleKit.Session IsLocal")

    machine.succeed("test -f /run/current-system/sw/share/polkit-1/actions/org.sessiond.consolekit-test.policy")
    machine.succeed("pkaction --verbose --action-id org.sessiond.consolekit-test")
    machine.succeed(f"pkcheck --process {pid} --action-id org.sessiond.consolekit-test")

    machine.shutdown()
  '';
}
