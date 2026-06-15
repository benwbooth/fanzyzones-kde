# FanzyZones KDE

FanzyZones KDE is a KDE Plasma version of FanzyZones: a FancyZones-style window
layout tool with a Plasma system-tray applet and a bundled KWin script. The Rust
backend owns installation, settings, layout selection, and KWin reloads. The KWin
script owns live window movement, drag overlays, keyboard shortcuts, virtual
desktops, and maximize/fullscreen handling.

## Status

This is an initial working project scaffold with real Rust code, an installable
Plasma 6 applet, an installable KWin package, shared JSON settings, and tests for
layout/config logic.

## Installation

FanzyZones KDE ships as a single **self-contained static binary** (x86_64) that
embeds the Plasma applet and KWin script and installs them itself. It needs
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

```sh
nix profile install github:benwbooth/fanzyzones-kde
fanzyzones-kde install --reload
```

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

There is no background daemon: the applet shells out to this CLI on demand. The
backend is pure Rust (no Qt) — the visual menu, settings, and layout editor all
run in the Plasma applet's own QML.

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
