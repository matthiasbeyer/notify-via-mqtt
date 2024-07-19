{
  description = "notify-via-mqtt";
  inputs = {
    nixpkgs.url = "nixpkgs/nixos-24.05";
    flake-utils.url = "github:numtide/flake-utils";
    crane = {
      url = "github:ipetkov/crane/v0.17.3";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs = {
        nixpkgs.follows = "nixpkgs";
      };
    };
  };

  outputs = inputs: inputs.flake-utils.lib.eachSystem [ "x86_64-linux" ]
    (system:
      let
        pkgs = import inputs.nixpkgs {
          inherit system;
          overlays = [
            (import inputs.rust-overlay)
          ];
        };

        nightlyRustTarget = pkgs.rust-bin.selectLatestNightlyWith (toolchain:
          pkgs.rust-bin.fromRustupToolchain { channel = "nightly-2024-02-07"; components = [ "rustfmt" ]; });

        nightlyCraneLib = (inputs.crane.mkLib pkgs).overrideToolchain nightlyRustTarget;
        rustTarget = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
        craneLib = (inputs.crane.mkLib pkgs).overrideToolchain rustTarget;

        tomlInfo = craneLib.crateNameFromCargoToml { cargoToml = ./Cargo.toml; };
        inherit (tomlInfo) version;
        pname = "notify-via-mqtt";

        src =
          let
            nixFilter = path: _type: !pkgs.lib.hasSuffix ".nix" path;
            extraFiles = path: _type: !(builtins.any (n: pkgs.lib.hasSuffix n path) [ ".github" ".sh" ]);
            filterPath = path: type: builtins.all (f: f path type) [
              nixFilter
              extraFiles
              pkgs.lib.cleanSourceFilter
            ];
          in
          pkgs.lib.cleanSourceWith {
            src = ./.;
            filter = filterPath;
          };

        buildInputs = [
          pkgs.pkg-config
          pkgs.openssl
        ];

        nativeBuildInputs = [
          pkgs.cmake
        ];

        cargoArtifacts = craneLib.buildDepsOnly {
          inherit src pname buildInputs nativeBuildInputs;
        };

        notify-via-mqtt = craneLib.buildPackage {
          inherit cargoArtifacts src pname version buildInputs nativeBuildInputs;
        };

        rustfmt' = pkgs.writeShellScriptBin "rustfmt" ''
          exec "${nightlyRustTarget}/bin/rustfmt" "$@"
        '';

        customCargoMultiplexer = pkgs.writeShellScriptBin "cargo" ''
          case "$1" in
            +nightly)
              shift
              export PATH="${nightlyRustTarget}/bin/:''$PATH"
              exec ${nightlyRustTarget}/bin/cargo "$@"
              ;;
            *)
              exec ${rustTarget}/bin/cargo "$@"
          esac
        '';
      in
      rec {
        checks = {
          inherit notify-via-mqtt;

          notify-via-mqtt-clippy = craneLib.cargoClippy {
            inherit cargoArtifacts src pname version buildInputs nativeBuildInputs;
            cargoClippyExtraArgs = "--benches --examples --tests --all-features -- --deny warnings";
          };

          notify-via-mqtt-fmt = nightlyCraneLib.cargoFmt {
            inherit src pname;
          };

          notify-via-mqtt-tests = craneLib.cargoNextest {
            inherit cargoArtifacts src pname version buildInputs nativeBuildInputs;
          };
        };

        packages = {
          default = packages.notify-via-mqtt;
          inherit notify-via-mqtt;
        };

        devShells.default = pkgs.mkShell {
          inherit buildInputs;

          nativeBuildInputs = nativeBuildInputs ++ [
            customCargoMultiplexer
            rustfmt'
            rustTarget

            pkgs.cargo-deny
            pkgs.gitlint
          ];
        };
      }
    );
}
