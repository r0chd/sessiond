{
  lib,
  pkgs,
  modulesPath,
  sessiond,
  sessiond-backlight,
  sessiond-hooks,
  sessiond-uaccess,
  ...
}:
let
  virtualTerminals = builtins.genList (index: "tty${toString (index + 1)}") 9;
in
{
  imports = [ "${modulesPath}/virtualisation/qemu-vm.nix" ];

  systemd.package = pkgs.systemd.override { withLogind = false; };

  services.sessiond = {
    enable = true;
    package = sessiond;
  };

  environment.systemPackages = [
    sessiond
    sessiond-backlight
    sessiond-hooks
    sessiond-uaccess
  ];

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
