{
  pkgs,
  sessiond,
  ...
}:
{
  services = {
    sysklogd.enable = true;
    sessiond = {
      enable = true;
      package = sessiond;
    };
    mdevd.enable = true;
    getty = {
      enable = true;
      ttys = [ "tty1" "tty2" "ttyS0" ];
    };
  };

  boot = {
    kernelParams = [ ];
    kernelModules = [
      "virtio_gpu"
      "virtio_input"
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
        "-vga"
        "virtio"
        "-device"
        "virtio-keyboard"
        "-usb"
        "-device"
        "usb-tablet,bus=usb-bus.0"
        "-monitor"
        "unix:/tmp/sessiond-vm-monitor.sock,server,nowait"
        "-serial"
        "unix:/tmp/sessiond-vm-serial.sock,server,nowait"
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
