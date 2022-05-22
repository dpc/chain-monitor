{
  description = "Chain Monitor";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs";
    flake-utils.url = "github:numtide/flake-utils";
    crane.url = "github:ipetkov/crane";
    crane.inputs.nixpkgs.follows = "nixpkgs";

    flake-compat = {
      url = "github:edolstra/flake-compat";
      flake = false;
    };

    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    naersk = {
      url = "github:nix-community/naersk";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, naersk, nixpkgs, flake-utils, flake-compat, fenix, crane }:
    flake-utils.lib.eachDefaultSystem (system:
    let
      pkgs = import nixpkgs {
        inherit system;
      };
      fenix-pkgs = fenix.packages.${system};
      fenix-channel = fenix-pkgs.stable;

      craneLib = (crane.mkLib pkgs).overrideScope' (final: prev: {
        # inherit (fenix-channel) cargo rustc;
      });

      src = ./.;

      cargoArtifacts = craneLib.buildDepsOnly {
        inherit src;
        buildInputs = [ pkgs.libsodium pkgs.lzma pkgs.openssl ];
        nativeBuildInputs = [
          pkgs.pkgconfig
          fenix-channel.rustc
        ];
      };

      chain-monitor = craneLib.buildPackage {
        inherit cargoArtifacts src;
      };

    in {
      defaultPackage = chain-monitor;

      devShell = pkgs.mkShell {

        buildInputs = cargoArtifacts.buildInputs;
        nativeBuildInputs = cargoArtifacts.nativeBuildInputs ++ [
          fenix-pkgs.rust-analyzer
          fenix-channel.rustfmt
          # fenix-channel.rustc
        ];
        RUST_SRC_PATH = "${fenix-channel.rust-src}/lib/rustlib/src/rust/library";
      };
  });
}
