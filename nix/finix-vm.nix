{
  pkgs,
  sessiond,
  seattest,
  ...
}:
let
  virtualTerminals = builtins.genList (index: "tty${toString (index + 1)}") 9;
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
{
  services = {
    polkit = {
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
    sysklogd.enable = true;
    sessiond = {
      enable = true;
      package = sessiond;
    };
    mdevd.enable = true;
    getty = {
      enable = true;
      ttys = virtualTerminals ++ [
        "ttyS0"
      ];
    };
  };

  boot = {
    kernelParams = [
      "console=ttyS0"
    ];
    kernelModules = [
      "usbhid"
      "hid_generic"
    ];
  };

  fileSystems = {
    "/" = {
      device = "tmpfs";
      fsType = "tmpfs";
      options = [ "mode=755" ];
    };

    "/tmp" = {
      device = "tmpfs";
      fsType = "tmpfs";
      options = [
        "mode=1777"
        "nosuid"
        "nodev"
      ];
    };
  };

  virtualisation = {
    memorySize = 2048;
    cores = 2;
    qemu = {
      extraArgs = [
        "-display"
        "gtk,gl=off"
        "-device"
        "virtio-vga"
        "-device"
        "virtio-tablet-pci"
        "-monitor"
        "unix:/tmp/sessiond-vm-monitor.sock,server,nowait"
        "-serial"
        "stdio"
      ];
      package = pkgs.qemu;
      nics.default.args = [
        "user"
        "model=virtio-net-pci"
        "hostfwd=tcp::2222-:22"
      ];
    };
  };

  finit.tmpfiles.rules = [
    "d /var/log 0755 root root -"
  ];

  environment.systemPackages = [
    polkitTestAction
    seattest.packages.default
  ];

  users.users = {
    user_1 = {
      isNormalUser = true;
      password = "";
      createHome = true;
      extraGroups = [ "wheel" ];
    };
    user_2 = {
      isNormalUser = true;
      password = "";
      createHome = true;
      extraGroups = [ "wheel" ];
    };
    root.password = "";
  };
}
