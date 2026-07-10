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
    };
  };

  testScript = ''
    machine.start()

    machine.wait_for_console_text("finix - stage 1")
    machine.wait_for_console_text("finix - stage 2")
    machine.wait_for_console_text("entering runlevel 2")

    machine.wait_until_succeeds("test -S /run/sessiond.sock")
    machine.succeed("test -x ${sessiondPackage}/bin/sessiond")
    machine.succeed("pam_path=$(awk '/pam_sessiond.so/ {print $3}' /etc/pam.d/login); test -e \"$pam_path\"")

    machine.wait_for_console_text("__SESSIOND_TEST_LOGIN_READY__:user_1")
    machine.succeed("sessionctl session list --json > /tmp/sessions.json")
    machine.succeed("jq -e '.sessions | length == 1' /tmp/sessions.json >/dev/null")
    machine.succeed("jq -e '.sessions[0].active == \"online\"' /tmp/sessions.json >/dev/null")
    machine.succeed("jq -e '.sessions[0].id == 1' /tmp/sessions.json >/dev/null")
    machine.succeed("jq -e '.sessions[0] | has(\"seat\") | not' /tmp/sessions.json >/dev/null")
    machine.succeed("jq -e '.sessions[0].uid == 1000' /tmp/sessions.json >/dev/null")
    machine.succeed("jq -e '.sessions[0].username == \"user_1\"' /tmp/sessions.json >/dev/null")
    machine.succeed("jq -e '.sessions[0] | has(\"vt\") | not' /tmp/sessions.json >/dev/null")
    machine.succeed("jq -e '.sessions[0] | has(\"tty\") | not' /tmp/sessions.json >/dev/null")
    machine.succeed("jq -e '.sessions[0].leader > 0' /tmp/sessions.json >/dev/null")
    machine.succeed("jq -e '.sessions[0].timestamp > 0' /tmp/sessions.json >/dev/null")

    machine.shutdown()
  '';
}
