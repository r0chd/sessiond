{
  pkgs,
  sessiond,
  ...
}:
let
  virtualTerminals = builtins.genList (index: "tty${toString (index + 1)}") 9;
in
{
  services = {
    polkit.enable = true;
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
      package = pkgs.qemu_full;
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
