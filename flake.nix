{
  inputs = {
    nixpkgs.url = "https://channels.nixos.org/nixos-unstable/nixexprs.tar.xz";
    crane.url = "github:ipetkov/crane";
  };

  outputs =
    {
      self,
      crane,
      nixpkgs,
      ...
    }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];

      sources = import ./npins;

      pkgsFor =
        system:
        import nixpkgs {
          localSystem = {
            inherit system;
            linux-kernel.target = "bzImage";
          };
        };

      forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f (pkgsFor system));

      finix = (import sources.finix.outPath) // {
        inherit (sources.finix) outPath;
      };
    in
    {
      devShells = forAllSystems (pkgs: {
        default =
          (import ./default.nix {
            inherit pkgs;
            craneLib = crane.mkLib pkgs;
          }).shell;
      });

      apps.x86_64-linux.finix-vm =
        let
          system = "x86_64-linux";
          pkgs = pkgsFor system;
          argv = self.finixConfigurations.finix-vm.config.virtualisation.qemu.argv;
          script = pkgs.writeShellScript "run-finix-vm" (nixpkgs.lib.escapeShellArgs argv);
        in
        {
          type = "app";
          program = "${script}";
        };

      packages = forAllSystems (
        pkgs:
        (import ./default.nix {
          inherit pkgs;
          craneLib = crane.mkLib pkgs;
        }).packages
      );

      nixosModules = {
        sessiond = import ./nix/sessiond/nixos.nix;
        default = self.nixosModules.sessiond;
        sessiond-backlight = import ./nix/sessiond-backlight/nixos.nix;
        sessiond-hooks = import ./nix/sessiond-hooks/nixos.nix;
        sessiond-uaccess = import ./nix/sessiond-uaccess/nixos.nix;
        sessiond-power = import ./nix/sessiond-power/nixos.nix;
      };

      finixModules = {
        sessiond = import ./nix/sessiond/finix.nix;
        default = self.finixModules.sessiond;
        sessiond-backlight = import ./nix/sessiond-backlight/finix.nix;
        sessiond-hooks = import ./nix/sessiond-hooks/finix.nix;
        sessiond-uaccess = import ./nix/sessiond-uaccess/finix.nix;
        sessiond-power = import ./nix/sessiond-power/finix.nix;
      };

      formatter = forAllSystems (pkgs: import ./nix/internal/formatter.nix { inherit pkgs; });

      nixosConfigurations =
        let
          system = "x86_64-linux";
        in
        {
          nixos-vm = nixpkgs.lib.nixosSystem {
            inherit system;
            specialArgs = {
              inherit (self.packages.${system})
                sessiond
                sessiond-backlight
                sessiond-hooks
                sessiond-uaccess
                sessiond-power
                ;
            };
            modules = [
              ./nix/sessiond/nixos.nix
              ./nix/sessiond-backlight/nixos.nix
              ./nix/sessiond-hooks/nixos.nix
              ./nix/sessiond-uaccess/nixos.nix
              ./nix/sessiond-power/nixos.nix
              ./nix/internal/nixos-vm.nix
            ];
          };
        };

      finixConfigurations =
        let
          system = "x86_64-linux";
          pkgs = pkgsFor system;
        in
        {
          finix-vm = finix.lib.finixSystem {
            inherit (pkgs) lib;
            specialArgs = {
              inherit (self.packages.${system})
                sessiond
                sessiond-backlight
                sessiond-hooks
                sessiond-uaccess
                sessiond-power
                ;
            };
            modules = [
              { nixpkgs = { inherit pkgs; }; }
              ./nix/sessiond/finix.nix
              ./nix/sessiond-backlight/finix.nix
              ./nix/sessiond-hooks/finix.nix
              ./nix/sessiond-uaccess/finix.nix
              ./nix/sessiond-power/finix.nix
              ./nix/internal/finix-vm.nix
              "${finix.outPath}/modules/virtualisation/qemu.nix"
            ];
          };
        };

      checks = forAllSystems (
        pkgs:
        let
          checkArgs = {
            inherit self finix pkgs;
          };
        in
        (import ./nix/internal/finix-checks.nix checkArgs)
        // (import ./nix/internal/nixos-checks.nix checkArgs)
      );
    };
}
