{
  lib,
  pkgs,
  modulesPath,
  sessiond,
  sessiond-backlight,
  sessiond-hooks,
  sessiond-uaccess,
  sessiond-power,
  ...
}:
let
  virtualTerminals = builtins.genList (index: "tty${toString (index + 1)}") 9;
in
{
  imports = [ "${modulesPath}/virtualisation/qemu-vm.nix" ];

  systemd.package = pkgs.systemd.override { withLogind = false; };

  environment.systemPackages = [ pkgs.dinit ];

  services = {
    sessiond = {
      enable = true;
      package = sessiond;
      debug = true;
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
          "${pkgs.zzz}/bin/zzz"
          "-Z"
        ];
        suspend = [
          "${pkgs.zzz}/bin/zzz"
          "-z"
        ];
        hybrid-sleep = [
          "${pkgs.zzz}/bin/zzz"
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
      };
    };
    udev.extraRules = ''
      SUBSYSTEM=="mem", KERNEL=="null", TAG+="uaccess"
    '';
  };

  security.pam.services.login.rules.session.sessiond = {
    control = "optional";
    modulePath = "${sessiond}/lib/security/pam_sessiond.so";
    order = 900;
  };

  boot.kernelParams = [
    "console=ttyS0"
  ];

  systemd.targets.getty.wants = lib.mkForce (
    map (tty: "autovt@${tty}.service") virtualTerminals ++ [ "serial-getty@ttyS0.service" ]
  );

  virtualisation = {
    qemu.options = [
      "-display none"
      "-serial stdio"
    ];
    memorySize = 2048;
    forwardPorts = [
      {
        from = "host";
        host.port = 2222;
        guest.port = 22;
      }
    ];
  };

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

  system.stateVersion = "26.05";
}
