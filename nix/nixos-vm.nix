{
  lib,
  pkgs,
  modulesPath,
  sessiond,
  seattest,
  ...
}:
let
  virtualTerminals = builtins.genList (index: "tty${toString (index + 1)}") 9;
in
{
  imports = [ "${modulesPath}/virtualisation/qemu-vm.nix" ];

  environment.systemPackages = [ seattest.packages.default ];

  systemd.package = pkgs.systemd.override { withLogind = false; };

  services.sessiond = {
    enable = true;
    package = sessiond;
  };

  systemd.targets.getty.wants = lib.mkForce (map (tty: "autovt@${tty}.service") virtualTerminals);

  virtualisation = {
    qemu.options = [ "-vga virtio" ];
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
      extraGroups = [
        "wheel"
        "seat"
      ];
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
