{
  description = "Infiniloom - High-performance repository context generator for LLMs";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        cargoToml = builtins.fromTOML (builtins.readFile ./Cargo.toml);

        # Platform-specific dependencies
        darwinDeps = with pkgs; [
          libiconv
          darwin.apple_sdk.frameworks.Security
          darwin.apple_sdk.frameworks.SystemConfiguration
        ];
        linuxDeps = with pkgs; [];
      in
      {
        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "infiniloom";
          version = cargoToml.workspace.package.version;
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;

          nativeBuildInputs = with pkgs; [ pkg-config ];
          buildInputs = (if pkgs.stdenv.isDarwin then darwinDeps else linuxDeps);

          # Skip tests in Nix build (may need network/git)
          doCheck = false;

          meta = with pkgs.lib; {
            description = "High-performance repository context generator for LLMs";
            license = licenses.mit;
          };
        };

        devShells.default = pkgs.mkShell {
          inputsFrom = [ self.packages.${system}.default ];
          buildInputs = with pkgs; [
            rustc
            cargo
            rust-analyzer
            clippy
            rustfmt
            cargo-watch
            cargo-llvm-cov
          ];
        };
      });
}
