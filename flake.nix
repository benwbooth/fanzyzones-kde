{
  description = "FanzyZones KDE - FancyZones-style KWin layouts controlled from a Plasma applet";

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

        # The backend is pure Rust now (no Qt). This builds a normal (glibc)
        # package for Nix users; the portable static-musl artifact for other
        # distros is produced by the CI workflow (cargo --target musl), which is
        # far less friction than coaxing buildRustPackage onto pkgsStatic.
        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "rust-analyzer" ];
        };

        # CLI tools the wrapper puts on PATH at runtime (the binary shells out to
        # these — it does NOT link them): kpackagetool6, kwriteconfig6/kreadconfig6,
        # busctl, qdbus6, xdg-open.
        runtimeDeps = with pkgs; [
          kdePackages.kpackage
          kdePackages.kconfig
          kdePackages.qttools
          systemd
          xdg-utils
        ];

        installResources = ''
          mkdir -p $out/share/fanzyzones-kde
          cp -R kwin-script $out/share/fanzyzones-kde/kwin-script
          mkdir -p $out/share/plasma/plasmoids
          cp -R plasma-applet $out/share/plasma/plasmoids/com.benwbooth.fanzyzones
          mkdir -p $out/share/icons
          cp -R resources/icons/hicolor $out/share/icons/
          wrapProgram $out/bin/fanzyzones-kde \
            --prefix PATH : ${pkgs.lib.makeBinPath runtimeDeps} \
            --set FANZYZONES_KDE_KWIN_SCRIPT_DIR "$out/share/fanzyzones-kde/kwin-script" \
            --set FANZYZONES_KDE_PLASMOID_DIR "$out/share/plasma/plasmoids/com.benwbooth.fanzyzones" \
            --set FANZYZONES_KDE_ICON_THEME_DIR "$out/share/icons" \
            --set FANZYZONES_KDE_TRAY_ICON_SOURCE "$out/share/icons/hicolor/scalable/status/fanzyzones-kde.svg"
        '';
      in
      {
        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "fanzyzones-kde";
          version = "0.1.0";
          src = ./.;

          cargoLock.lockFile = ./Cargo.lock;

          nativeBuildInputs = [ pkgs.makeWrapper ];

          postInstall = installResources;

          meta = with pkgs.lib; {
            description = "KDE Plasma applet and KWin script for FancyZones-style window layouts";
            license = licenses.mit;
            platforms = platforms.linux;
            mainProgram = "fanzyzones-kde";
          };
        };

        devShells.default = pkgs.mkShell {
          packages = [ rustToolchain pkgs.pkg-config ] ++ runtimeDeps ++ (with pkgs; [
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
            export FANZYZONES_KDE_PLASMOID_DIR="$PWD/plasma-applet"
            export FANZYZONES_KDE_ICON_THEME_DIR="$PWD/resources/icons"
            export FANZYZONES_KDE_TRAY_ICON_SOURCE="$PWD/resources/icons/hicolor/scalable/status/fanzyzones-kde.svg"
          '';
        };
      });
}
