{
  lib,
  pkgs,
  config,
  modules,
  sessiond,
  sessiond-hooks,
  sessiond-uaccess,
  sessiond-backlight,
  sessiond-power,
  ...
}:
let
  virtualTerminals = builtins.genList (index: "tty${toString (index + 1)}") 9;
in
{
  imports = [
    modules.zzz
    modules.doas
    modules.getty
    modules.sysklogd
  ];

  environment.etc."dinit.d/user/boot".text = ''
    type = internal
  '';

  environment.etc."systemd/user".source = "${pkgs.systemd}/example/systemd/user";

  programs.doas.enable = true;
  providers.privileges.rules = [
    {
      groups = [ "wheel" ];
      command = "ALL";
      runAs = "*";
      requirePassword = false;
    }
  ];

  services = {
    polkit.enable = true;
    sysklogd.enable = true;
    sessiond = {
      enable = true;
      package = sessiond;
      debug = true;
      settings = {
        users.user_3.linger = true;
      };
    };
    sessiond-uaccess = {
      enable = true;
      package = sessiond-uaccess;
      debug = true;
    };
    sessiond-backlight = {
      enable = true;
      package = sessiond-backlight;
      debug = true;
    };
    sessiond-power = {
      enable = true;
      package = sessiond-power;
      debug = true;
      settings = {
        reboot = [ "/run/current-system/sw/bin/reboot" ];
        poweroff = [ "/run/current-system/sw/bin/poweroff" ];
        hibernate = [
          "${config.programs.zzz.package}/bin/zzz"
          "-Z"
        ];
        suspend = [
          "${config.programs.zzz.package}/bin/zzz"
          "-z"
        ];
        hybrid-sleep = [
          "${config.programs.zzz.package}/bin/zzz"
          "-H"
        ];
      };
    };
    sessiond-hooks = {
      package = sessiond-hooks;
      debug = true;
      hooks = {
        systemd.settings = {
          on_start = "${pkgs.systemd}/lib/systemd/systemd --user";
          restart = {
            on = "Always";
            delay = 5;
          };
        };
        dinit.settings = {
          on_start = "${lib.getExe' pkgs.dinit "dinit"} --user --services-dir /etc/dinit.d/user";
          restart = {
            on = "Failure";
            delay = 5;
          };
        };
      };
    };
    keventd.enable = true;
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
    "d /run/systemd/system 0755 root root -"
  ];

  environment.systemPackages = [
    pkgs.dinit
    pkgs.bashInteractive
    pkgs.systemd
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
    user_3 = {
      isNormalUser = true;
      password = "";
      createHome = true;
      extraGroups = [ "wheel" ];
    };
    root.password = "";
  };
}
