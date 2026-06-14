# FanzyZones KDE

FanzyZones KDE is a KDE Plasma version of FanzyZones: a FancyZones-style window
layout tool with a tray controller and a bundled KWin script. The Rust tray app
owns installation, settings, layout selection, and KWin reloads. The KWin script
owns live window movement, drag overlays, keyboard shortcuts, virtual desktops,
and maximize/fullscreen handling.

## Status

This is an initial working project scaffold with real Rust code, an installable
Plasma 6 KWin package, shared JSON settings, and tests for layout/config logic.

## FanzyZones Parity

- Seven built-in FanzyZones layouts:
  - Two Panes
  - Two Panes (Wide + Side)
  - Three Panes
  - Three Panes (Ultrawide)
  - Quarters
  - Priority (Left Focus)
  - Grid 3x3
- Tray icon with an original-style visual layout menu: click a layout name to
  activate it, or click a pane in its mini diagram to move the focused window
  there.
- First-run tray setup that idempotently installs or upgrades the bundled KWin
  script, writes settings, enables it, and asks KWin to reconfigure.
- Focused-window snapping to zone 1 through 9.
- Focused-window previous/next zone cycling.
- Drag-to-snap with overlay.
- Modifier-gated drag snapping or auto-snap mode.
- Configurable gap, outer padding, overlay color, opacity, and zone numbers.
- Per-screen and per-desktop active layout tracking.
- Custom layouts through imported or edited JSON settings, with visual-menu
  create/edit/delete hooks for user layouts.
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
`kpackagetool6`, `kwriteconfig6`, and `qdbus6`.

## Commands

```sh
fanzyzones-kde tray
fanzyzones-kde visual-menu
fanzyzones-kde install --reload
fanzyzones-kde write-config
fanzyzones-kde reload-kwin
fanzyzones-kde print-config
fanzyzones-kde config-path
fanzyzones-kde set-layout builtin.priority-left --sync
fanzyzones-kde snap-zone 1
fanzyzones-kde import-config ./settings.json --sync
fanzyzones-kde disable
```

Running `fanzyzones-kde tray` is enough for normal use; the tray app performs
the same setup work as `fanzyzones-kde install --reload` on startup and reports
setup errors in the tray menu.

Left-clicking the tray icon opens the visual FanzyZones menu. Right-clicking
opens the plain tray command menu.

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
