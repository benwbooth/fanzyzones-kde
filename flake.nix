{
  description = "FanzyZones KDE - FancyZones-style KWin layouts controlled from a Rust tray app";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "rust-analyzer" ];
        };

        runtimeDeps = with pkgs; [
          kdePackages.kpackage
          kdePackages.kconfig
          kdePackages.kwindowsystem
          qt6.qttools
          xdg-utils
        ];

        nativeBuildInputs = with pkgs; [
          rustToolchain
          pkg-config
        ];
      in
      {
        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "fanzyzones-kde";
          version = "0.1.0";
          src = ./.;

          cargoLock = {
            lockFile = ./Cargo.lock;
          };

          nativeBuildInputs = nativeBuildInputs ++ [ pkgs.makeWrapper ];

          buildInputs = with pkgs; [
            dbus
          ];

          postInstall = ''
            mkdir -p $out/share/fanzyzones-kde
            cp -R kwin-script $out/share/fanzyzones-kde/kwin-script
            mkdir -p $out/share/icons
            cp -R resources/icons/hicolor $out/share/icons/
            wrapProgram $out/bin/fanzyzones-kde \
              --prefix PATH : ${pkgs.lib.makeBinPath runtimeDeps} \
              --set FANZYZONES_KDE_KWIN_SCRIPT_DIR "$out/share/fanzyzones-kde/kwin-script" \
              --set FANZYZONES_KDE_ICON_THEME_DIR "$out/share/icons"
          '';

          meta = with pkgs.lib; {
            description = "KDE tray app and KWin script for FancyZones-style window layouts";
            license = licenses.mit;
            platforms = platforms.linux;
            mainProgram = "fanzyzones-kde";
          };
        };

        devShells.default = pkgs.mkShell {
          packages = nativeBuildInputs ++ runtimeDeps ++ (with pkgs; [
            cargo-watch
            clippy
            rust-analyzer
            rustfmt
          ]);

          RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";

          shellHook = ''
            echo "FanzyZones KDE development environment"
            echo "Run 'cargo test' or 'cargo run -- install --reload'"
            export FANZYZONES_KDE_KWIN_SCRIPT_DIR="$PWD/kwin-script"
            export FANZYZONES_KDE_ICON_THEME_DIR="$PWD/resources/icons"
          '';
        };
      });
}
