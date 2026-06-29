{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
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
          inherit system;
          overlays = [ rust-overlay.overlays.default ];
        };

      forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f (pkgsFor system));
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
          in
          import ./nix/shell.nix { inherit pkgs rustToolchain; };
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

      packages = forAllSystems (pkgs: {
        sessiond = pkgs.callPackage ./nix/package.nix {
          rustPlatform =
            let
              rustToolchain = pkgs.rust-bin.stable.latest.default.override {
                extensions = [
                  "rust-analyzer"
                  "rust-src"
                  "llvm-tools"
                ];
              };
            in
            pkgs.makeRustPlatform {
              cargo = rustToolchain;
              rustc = rustToolchain;
            };
        };
        default = self.packages.${pkgs.stdenv.hostPlatform.system}.sessiond;
      });

      nixosModules = {
        sessiond = import ./nix/nixos.nix;
        default = self.nixosModules.sessiond;
      };

      finixModules = {
        sessiond = import ./nix/finix.nix;
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
        import ./nix/formatter.nix { inherit pkgs rustToolchain; }
      );

      nixosConfigurations =
        let
          system = "x86_64-linux";
          pkgs = pkgsFor system;
          seattest = (import sources.seattest.outPath {
            inherit pkgs;
          }) // {
            inherit (sources.seattest) outPath;
          };
        in
        {
          nixos-vm = nixpkgs.lib.nixosSystem {
            inherit system;
            specialArgs = {
              inherit (self.packages.${system}) sessiond;
              inherit seattest;
            };
            modules = [
              ./nix/nixos.nix
              ./nix/nixos-vm.nix
            ];
          };
        };

      finixConfigurations =
        let
          system = "x86_64-linux";
          pkgs = pkgsFor system;
          seattest = (import sources.seattest.outPath {
            inherit pkgs;
          }) // {
            inherit (sources.seattest) outPath;
          };
          finix = (import sources.finix.outPath) // {
            inherit (sources.finix) outPath;
          };
        in
        {
          finix-vm = finix.lib.finixSystem {
            inherit (pkgs) lib;
            specialArgs = {
              inherit (self.packages.${system}) sessiond;
              inherit seattest;
            };
            modules = [
              { nixpkgs = { inherit pkgs; }; }
              ./nix/finix-vm.nix
              ./nix/finix.nix
              "${finix.outPath}/modules/virtualisation/qemu.nix"
            ]
            ++ builtins.attrValues {
              inherit (finix.nixosModules)
                getty
                sysklogd
                ;
            };
          };
        };
    };
}
