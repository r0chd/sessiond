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
  imports = [ modules.zzz ];
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
      hooks.dinit.settings = {
        on_start = {
          exec = lib.getExe' pkgs.dinit "dinit";
          args = [
            "--user"
            "--services-dir"
            "~/.config/dinit.d"
          ];
        };
        restart = {
          on = "Failure";
          delay = 5;
        };
      };
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
    pkgs.systemd
    polkitTestAction
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

  security.pam.services.login.text = lib.mkAfter ''
    session   optional   ${config.services.sessiond.package}/lib/security/pam_sessiond.so
  '';
}
