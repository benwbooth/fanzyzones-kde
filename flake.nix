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
        # musl cross toolchain so `cargo build --target x86_64-unknown-linux-musl`
        # produces the portable static binary locally (same as the CI release).
        muslPkgs = pkgs.pkgsCross.musl64;
        muslCc = "${muslPkgs.stdenv.cc}/bin/${muslPkgs.stdenv.cc.targetPrefix}cc";

        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "rust-analyzer" ];
          targets = [ "x86_64-unknown-linux-musl" ];
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
          version = "0.1.3";
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

          # Static musl release build: cargo build --release --target x86_64-unknown-linux-musl
          CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER = muslCc;
          CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_RUSTFLAGS = "-C target-feature=+crt-static";

          shellHook = ''
            echo "FanzyZones KDE development environment"
            echo "Run 'cargo test' or 'cargo run -- install --reload'"
            export FANZYZONES_KDE_KWIN_SCRIPT_DIR="$PWD/kwin-script"
            export FANZYZONES_KDE_PLASMOID_DIR="$PWD/plasma-applet"
            export FANZYZONES_KDE_ICON_THEME_DIR="$PWD/resources/icons"
            export FANZYZONES_KDE_TRAY_ICON_SOURCE="$PWD/resources/icons/hicolor/scalable/status/fanzyzones-kde.svg"
          '';
        };
      })
      // {
        # Home Manager module — declarative install. FanzyZones' Plasma/KWin
        # integration is stateful (kwinrc settings, the KWin script, the tray
        # item, kpackagetool registration), so it can't live purely in the Nix
        # store; the module installs the package and runs its installer on
        # activation so there's no manual `install` step.
        homeManagerModules.default = { config, lib, pkgs, ... }:
          let
            cfg = config.programs.fanzyzones-kde;
          in
          {
            options.programs.fanzyzones-kde = {
              enable =
                lib.mkEnableOption "FanzyZones KDE (FancyZones-style tiling for Plasma 6)";
              package = lib.mkOption {
                type = lib.types.package;
                default = self.packages.${pkgs.stdenv.hostPlatform.system}.default;
                defaultText = lib.literalExpression "fanzyzones-kde.packages.\${system}.default";
                description = "The fanzyzones-kde package to install.";
              };
            };

            config = lib.mkIf cfg.enable {
              home.packages = [ cfg.package ];

              # Best-effort: register the applet/KWin script + write settings and
              # reload. `|| true` so a switch from a TTY (no live session to
              # reload) never fails; the install/config steps still apply and
              # take effect on the next Plasma session.
              home.activation.fanzyzonesKde =
                lib.hm.dag.entryAfter [ "writeBoundary" ] ''
                  run ${lib.getExe cfg.package} install --reload || true
                '';
            };
          };
      };
}
