{
  modulesPath,
  moxsession,
  ...
}:
{
  imports = [ "${modulesPath}/virtualisation/qemu-vm.nix" ];

  services.moxsession = {
    enable = true;
    package = moxsession;
  };

  virtualisation = {
    qemu.options = [ "-vga virtio" ];
    # graphics = false;
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
