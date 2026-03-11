# Nix Syntax Highlighting Test
# A flake-based development environment and package definition.

{
  description = "Token Editor - a minimal text editor in Rust";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crane = {
      url = "github:ipetkov/crane";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay, crane }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        # Rust toolchain from rust-toolchain.toml
        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "rust-analyzer" "clippy" ];
          targets = [ "x86_64-unknown-linux-gnu" "aarch64-apple-darwin" ];
        };

        craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;

        # Common arguments for crane builds
        commonArgs = {
          src = craneLib.cleanCargoSource ./.;
          strictDeps = true;

          buildInputs = with pkgs; [
            # System libraries
            fontconfig
            freetype
            libxkbcommon
            wayland
          ] ++ lib.optionals stdenv.isDarwin [
            darwin.apple_sdk.frameworks.AppKit
            darwin.apple_sdk.frameworks.CoreGraphics
            darwin.apple_sdk.frameworks.CoreText
            darwin.apple_sdk.frameworks.Metal
            darwin.apple_sdk.frameworks.QuartzCore
          ];

          nativeBuildInputs = with pkgs; [
            pkg-config
            cmake
          ];
        };

        # Build just the cargo dependencies
        cargoArtifacts = craneLib.buildDepsOnly commonArgs;

        # The main package
        token-editor = craneLib.buildPackage (commonArgs // {
          inherit cargoArtifacts;

          # Additional metadata
          pname = "token-editor";
          version =
            let
              cargoToml = builtins.fromTOML (builtins.readFile ./Cargo.toml);
            in
              cargoToml.package.version;

          meta = with pkgs.lib; {
            description = "A minimal text editor implementing the Elm Architecture";
            homepage = "https://github.com/example/token-editor";
            license = licenses.mit;
            maintainers = [ ];
            platforms = platforms.unix;
          };
        });

        # Helper function to create a theme package
        mkTheme = name: src: pkgs.stdenv.mkDerivation {
          pname = "token-editor-theme-${name}";
          version = "1.0.0";
          inherit src;

          installPhase = ''
            mkdir -p $out/share/token-editor/themes
            cp *.yaml $out/share/token-editor/themes/
          '';
        };

        # Development tools
        devTools = with pkgs; [
          # Rust
          rustToolchain
          cargo-watch
          cargo-nextest
          cargo-flamegraph
          cargo-deny

          # Build tools
          gnumake
          cmake
          pkg-config

          # Debug & profiling
          lldb
          samply
          heaptrack

          # Formatting
          treefmt
          nixpkgs-fmt
        ];

      in
      {
        # Packages
        packages = {
          default = token-editor;
          inherit token-editor;

          # Docker image
          docker = pkgs.dockerTools.buildLayeredImage {
            name = "token-editor";
            tag = "latest";
            contents = [ token-editor ];
            config = {
              Entrypoint = [ "${token-editor}/bin/token" ];
              WorkingDir = "/workspace";
            };
          };
        };

        # Development shell
        devShells.default = craneLib.devShell {
          checks = self.checks.${system};

          packages = devTools;

          # Environment variables
          RUST_BACKTRACE = "1";
          RUST_LOG = "token=debug";

          shellHook = ''
            echo "Token Editor development environment"
            echo "Rust: $(rustc --version)"
            echo ""
            echo "Commands:"
            echo "  make build    - Build debug binary"
            echo "  make test     - Run tests"
            echo "  make dev      - Run in development mode"
            echo ""
          '';
        };

        # CI checks
        checks = {
          inherit token-editor;

          # Clippy lints
          token-clippy = craneLib.cargoClippy (commonArgs // {
            inherit cargoArtifacts;
            cargoClippyExtraArgs = "--all-targets -- -D warnings";
          });

          # Tests
          token-tests = craneLib.cargoNextest (commonArgs // {
            inherit cargoArtifacts;
            partitions = 1;
            partitionType = "count";
          });

          # Formatting
          token-fmt = craneLib.cargoFmt {
            src = ./.;
          };

          # Audit
          token-audit = craneLib.cargoAudit {
            src = ./.;
            advisory-db = pkgs.fetchFromGitHub {
              owner = "rustsec";
              repo = "advisory-db";
              rev = "main";
              sha256 = pkgs.lib.fakeSha256;
            };
          };
        };

        # Formatter
        formatter = pkgs.treefmt;

        # NixOS module
        nixosModules.default = { config, lib, pkgs, ... }:
          with lib;
          let
            cfg = config.programs.token-editor;
          in
          {
            options.programs.token-editor = {
              enable = mkEnableOption "Token Editor";

              package = mkOption {
                type = types.package;
                default = self.packages.${system}.default;
                description = "The token-editor package to use.";
              };

              theme = mkOption {
                type = types.str;
                default = "dark";
                description = "Default theme name.";
              };

              settings = mkOption {
                type = types.attrsOf types.anything;
                default = { };
                example = literalExpression ''
                  {
                    font_size = 14;
                    tab_width = 4;
                    line_numbers = true;
                  }
                '';
                description = "Configuration settings for token-editor.";
              };
            };

            config = mkIf cfg.enable {
              environment.systemPackages = [ cfg.package ];

              xdg.configFile."token-editor/config.yaml".text =
                builtins.toJSON ({
                  theme = cfg.theme;
                } // cfg.settings);
            };
          };
      });
}
