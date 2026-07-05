{
  pkgs,
  sessiondTest,
  sessiondModule,
  sessiondPackage,
}:
let
  busctl = "${pkgs.systemd}/bin/busctl";
in
sessiondTest {
  name = "sessiond-consolekit-finix";

  nodes.machine = {
    imports = [ sessiondModule ];

    services.dbus.enable = true;
    services.mdevd.enable = true;
    services.sessiond = {
      enable = true;
      package = sessiondPackage;
    };
  };

  testScript = ''
    machine.start()

    machine.wait_for_console_text("finix - stage 1")
    machine.wait_for_console_text("finix - stage 2")
    machine.wait_for_console_text("entering runlevel 2")

    machine.wait_until_succeeds("test -S /run/sessiond.sock")
    machine.wait_until_succeeds("test -S /run/dbus/system_bus_socket")

    machine.succeed("${busctl} --system status org.freedesktop.ConsoleKit")
    machine.succeed("${busctl} --system call org.freedesktop.ConsoleKit /org/freedesktop/ConsoleKit/Manager org.freedesktop.ConsoleKit.Manager GetSessionForUnixProcess u 1")

    machine.shutdown()
  '';
}
