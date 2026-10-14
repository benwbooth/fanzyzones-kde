# FanzyZones KDE

FanzyZones KDE is a KDE Plasma version of FanzyZones: a FancyZones-style window
layout tool with a Plasma applet, a KWin zone-layout script, and an optional
Trellis-style nested workspace. Dock windows into nested spaces, group different
apps into tabs, resize their seams, and zoom between views while continuing to
work in the real application windows.

## Status

Version 0.3 adds nested workspaces alongside the existing zone layouts. The
nested workspace targets Plasma 6.6+ on Wayland with OpenGL compositing. See
[the feature and validation matrix](docs/trellis-parity.md) for coverage and
platform requirements.

## Nested workspace

After installing, choose **Enable nested workspace** from the applet and follow
the setup wizard, or run:

```sh
fanzyzones-kde workspace enable
fanzyzones-kde workspace arrange
```

- Dock by dragging a title strip to a window or screen edge. Drop in the middle
  after a brief pause to form tabs; drag a tab out to detach it.
- Drag seams to resize adjacent spaces. Shift-drag a tab to move its whole group.
- Use `Ctrl+Alt+Space` for everything, `Ctrl+Alt+Return` for one window,
  `Ctrl+Alt+Up/Down` for levels, and `Ctrl+Alt+[/]` for view history.
- Save nine views per display with `Ctrl+Alt+Shift+1` through `9`; recall them
  with `Ctrl+Alt+1` through `9`.
- With the matching native input module, hold Alt and pinch with two fingers,
  pinch on bare Desktop, or hold Meta+Shift and drag a rectangle. Three-finger
  pinch and the `Meta+Shift+R` framing shortcut also work without that module.
- Pinch continuously through nested spaces; release to settle on the best-fitting
  view. Double-click a native title bar to focus, or click its maximize button to
  dock/float (Alt-click keeps native maximize).
- Reopen setup, check for updates, or prepare an editable problem-report draft
  from the applet menu.
- Disable the workspace to restore normal window positions and decorations:
  `fanzyzones-kde workspace disable`. The emergency shortcut is
  `Meta+Ctrl+Shift+F12`.

Nix packages include the native input module. Keep the flake's nixpkgs input
aligned with your desktop, as the Home Manager example below does. Portable
Rust binaries need the [separately built module](docs/trellis-parity.md#native-input-module)
for two-finger gestures and modifier-drag framing.

## Installation

FanzyZones KDE ships as a single **self-contained static binary** (x86_64) that
embeds the Plasma applet, KWin script and workspace effect and installs them itself. It needs
**KDE Plasma 6** (Wayland or X11).

### Quick install

```sh
curl -fsSL https://raw.githubusercontent.com/benwbooth/fanzyzones-kde/main/install.sh | sh
```

This downloads the latest release binary to `~/.local/bin/fanzyzones-kde` and
runs its installer. Re-run it any time to update.

### Manual

Grab the binary from the
[latest release](https://github.com/benwbooth/fanzyzones-kde/releases/latest):

```sh
curl -fSLO https://github.com/benwbooth/fanzyzones-kde/releases/latest/download/fanzyzones-kde-x86_64-linux
chmod +x fanzyzones-kde-x86_64-linux
./fanzyzones-kde-x86_64-linux install --reload
```

### Nix

Imperative:

```sh
nix profile install github:benwbooth/fanzyzones-kde
fanzyzones-kde install --reload
```

Declarative with Home Manager — add the flake input and import the module:

```nix
# flake.nix inputs
inputs.fanzyzones-kde = {
  url = "github:benwbooth/fanzyzones-kde";
  inputs.nixpkgs.follows = "nixpkgs";
};

# Home Manager configuration
imports = [ inputs.fanzyzones-kde.homeManagerModules.default ];
programs.fanzyzones-kde.enable = true;
```

The module adds the package and runs its installer on activation (registers the
applet + KWin script, writes settings, adds the tray item) — no manual step.
FanzyZones' Plasma/KWin integration is per-user mutable state, so this is the
declarative way to wire it in; it takes effect in your next Plasma session.

### From source

```sh
nix develop            # or supply Rust + the KDE/Qt CLI tools yourself
cargo build --release
./target/release/fanzyzones-kde install --reload
```

Installation adds the **FanzyZones** widget to your system tray automatically.
(If it doesn't appear, right-click the tray → Configure System Tray and enable
"FanzyZones".) The in-app **Settings** and the tray menu's **New Custom Layout**
open from the tray icon.

## FanzyZones Parity

- Seven built-in FanzyZones layouts:
  - Two Panes
  - Two Panes (Wide + Side)
  - Three Panes
  - Three Panes (Ultrawide)
  - Quarters
  - Priority (Left Focus)
  - Grid 3x3
- Plasma tray icon with an original-style visual layout menu: click a layout name to
  activate it, or click a pane in its mini diagram to move the focused window
  there.
- Idempotent setup that installs or upgrades the Plasma applet and the bundled
  KWin script, writes settings, enables it, and asks KWin to reconfigure.
- Focused-window snapping to zone 1 through 9.
- Focused-window previous/next zone cycling.
- Drag-to-snap with overlay.
- Modifier-gated drag snapping or auto-snap mode.
- Configurable gap, outer padding, overlay color, opacity, and zone numbers.
- Per-screen and per-desktop active layout tracking.
- Custom layouts via a built-in visual editor (drag/resize/split zones, name,
  save) reachable from the tray menu, plus create/edit/delete of user layouts.
- Per-display layout assignment with a tray display picker, and native KWin
  Shift+drag tiling driven by the active layout.
- Keyboard shortcuts matching the macOS behavior concept:
  - `Ctrl+Alt+Left` / `Ctrl+Alt+Right` cycle zones.
  - `Ctrl+Alt+Num+1` through `Ctrl+Alt+Num+9` snap to a zone.
- Settings persisted as JSON under the XDG config directory.

## KDE-Specific Behavior

- Zone selector at the top of the screen while dragging.
- Optional edge snapping.
- Auto-snap newly created windows to their closest zone.
- Dynamic workspaces with a trailing empty desktop.
- MACsimize-style dedicated desktops for fullscreen or maximized windows.
- Transient windows follow their parent window's dedicated desktop.
- Non-maximized windows opened on a managed fullscreen/maximized desktop are sent
  back to the main desktop when exclusive desktop mode is enabled.

## Development

```sh
direnv allow
cargo test
cargo run -- print-config
cargo run -- install --reload
```

The dev shell provides Rust plus KDE/Qt command-line tools such as
`kpackagetool6`, `kwriteconfig6`, and `busctl`.

## Commands

```sh
fanzyzones-kde install --reload
fanzyzones-kde install-plasmoid
fanzyzones-kde state-json --sync
fanzyzones-kde invoke-action '{"action":"setLayout","layout":0,"closeMenu":false}'
fanzyzones-kde write-config
fanzyzones-kde reload-kwin
fanzyzones-kde print-config
fanzyzones-kde config-path
fanzyzones-kde set-layout builtin.priority-left --sync
fanzyzones-kde snap-zone 1
fanzyzones-kde import-config ./settings.json --sync
fanzyzones-kde disable
```

Running `fanzyzones-kde install --reload` is enough for normal use. It unpacks
the embedded plasmoid + KWin script (when running as a standalone binary),
installs/upgrades them with `kpackagetool6`, writes a small CLI wrapper the
applet calls, syncs settings into `kwinrc`, and reloads the KWin script.

The applet calls the CLI on demand. A small D-Bus activated Rust service stores
workspace state atomically and accepts commands; KWin owns all live window
operations. Menus and settings use Plasma QML. Native gestures use the optional
module compiled for the exact installed KWin version.

## Layout JSON

Layouts use the same normalized top-left-origin model as FanzyZones. Coordinates
are `0.0..1.0`, so a full-height left half zone is:

```json
{
  "id": 0,
  "name": "Left",
  "x": 0.0,
  "y": 0.0,
  "width": 0.5,
  "height": 1.0,
  "applications": []
}
```

Use `fanzyzones-kde print-config > settings.json`, edit or add layouts, then
import them with `fanzyzones-kde import-config settings.json --sync`.
