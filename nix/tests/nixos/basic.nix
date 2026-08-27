{
  pkgs,
  sessiondTest,
  sessiondModule,
  sessiondPackage,
}:
sessiondTest {
  name = "sessiond-basic-nixos";

  nodes.machine = {
    imports = [ sessiondModule ];

    environment.systemPackages = [ pkgs.jq ];

    programs.bash.loginShellInit = ''
      if [ "$(tty)" = /dev/tty1 ]; then
        touch /tmp/sessiond-test-login-ready
      fi
    '';

    services.getty.autologinUser = "user_1";
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
    };
  };

  testScript = ''
    machine.start()

    machine.wait_for_unit("sessiond.service")

    machine.succeed("chvt 1")
    machine.wait_for_unit("getty@tty1.service")
    machine.wait_for_file("/tmp/sessiond-test-login-ready")
    machine.wait_until_succeeds("sessionctl session list --json > /tmp/sessions.json && jq -e '.sessions[] | select(.uid == 1000 and .username == \"user_1\" and .vt == 1 and .tty == \"tty1\")' /tmp/sessions.json >/dev/null")

    machine.shutdown()
  '';
}
