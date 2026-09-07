{
  inputs = {
    nixpkgs.url = "https://channels.nixos.org/nixos-unstable/nixexprs.tar.xz";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
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
          overlays = [ rust-overlay.overlays.default ];
        };

      forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f (pkgsFor system));

      finix = (import sources.finix.outPath) // {
        inherit (sources.finix) outPath;
      };
    in
    {
      devShells = forAllSystems (pkgs: {
        default =
          let
            rustToolchain = pkgs.rust-bin.stable.latest.default.override {
              extensions = [
                "rust-analyzer"
                "rust-src"
              ];
            };
            sessiond = self.packages.${pkgs.stdenv.hostPlatform.system}.sessiond;
          in
          import ./nix/internal/shell.nix { inherit pkgs rustToolchain sessiond; };
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
        let
          rustToolchain = pkgs.rust-bin.stable.latest.default.override {
            extensions = [
              "rust-analyzer"
              "rust-src"
              "llvm-tools"
            ];
          };
          rustPlatform = pkgs.makeRustPlatform {
            cargo = rustToolchain;
            rustc = rustToolchain;
          };
        in
        {
          sessiond = pkgs.callPackage ./nix/sessiond/package.nix {
            inherit rustPlatform;
          };
          sessiond-backlight = pkgs.callPackage ./nix/sessiond-backlight/package.nix {
            inherit rustPlatform;
          };
          sessiond-hooks = pkgs.callPackage ./nix/sessiond-hooks/package.nix {
            inherit rustPlatform;
          };
          sessiond-uaccess = pkgs.callPackage ./nix/sessiond-uaccess/package.nix {
            inherit rustPlatform;
          };
          default = self.packages.${pkgs.stdenv.hostPlatform.system}.sessiond;
        }
      );

      nixosModules = {
        sessiond = import ./nix/sessiond/nixos.nix;
        default = self.nixosModules.sessiond;
      };

      finixModules = {
        sessiond = import ./nix/sessiond/finix.nix;
        sessiond-backlight = import ./nix/sessiond-backlight/finix.nix;
        sessiond-hooks = import ./nix/sessiond-hooks/finix.nix;
        sessiond-uaccess = import ./nix/sessiond-uaccess/finix.nix;
        default = self.finixModules.sessiond;
      };

      formatter = forAllSystems (
        pkgs:
        let
          rustToolchain = pkgs.rust-bin.stable.latest.default.override {
            extensions = [
              "rust-analyzer"
              "rust-src"
            ];
          };
        in
        import ./nix/internal/formatter.nix { inherit pkgs rustToolchain; }
      );

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
                ;
            };
            modules = [
              ./nix/sessiond/nixos.nix
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
                ;
            };
            modules = [
              { nixpkgs = { inherit pkgs; }; }
              ./nix/sessiond/finix.nix
              ./nix/sessiond-backlight/finix.nix
              ./nix/sessiond-hooks/finix.nix
              ./nix/sessiond-uaccess/finix.nix
              ./nix/internal/finix-vm.nix
              "${finix.outPath}/modules/virtualisation/qemu.nix"
            ]
            ++ builtins.attrValues {
              inherit (finix.nixosModules)
                getty
                sysklogd
                polkit
                ;
            };
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
