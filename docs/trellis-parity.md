# Nested workspace and TrellisWM parity

The implementation follows the interaction model described on
[TrellisWM's feature page](https://trelliswm.app/#features) and
[guide](https://trelliswm.app/docs/), reviewed on 2026-10-14. It is an independent
KWin implementation. Existing fixed zone layouts remain available; nested
workspaces are opt-in.

## Feature coverage

| Trellis interaction | FanzyZones implementation |
| --- | --- |
| Nested docking, window and screen edges | Recursive splits, edge drop previews, floating Desktop |
| Cross-app tabs | Center dwell, tab selection, tear-off, context menu; Shift-drag moves the group |
| Resize at seams | Adjacent weights change together; unrelated spaces retain their sizes |
| Small windows and groups | App icon and color placeholders; group split miniatures; double-click to focus |
| Zoom, focus, framing | Live thumbnails, continuous camera scaling, best-fitting space on release, hierarchy, neighbour navigation, history and rectangle framing |
| Trackpad interaction | Alt+two-finger pinch or bare-Desktop pinch with the native module, including reversal and cancellation; three-finger fallback |
| Title and Desktop focus | Double-click a native KWin title bar, managed title strip or bare Desktop; floating-window focus and return |
| Float / dock button | Native maximize button toggles docking; Alt retains native maximize. Managed strips have the same actions |
| Nine saved views | Independent slots 1–9 on each display and virtual desktop |
| Menu-bar controls | Plasma applet navigation and view indicators; optional expanded controls when placed directly in a panel |
| Minimize and return | Reflow while minimized; original split and weights remain available on restore |
| Restart persistence | Atomic snapshot plus backup, stable display/desktop identity, window matching on reopen |
| Multiple displays | Independent cameras, drag between displays, fallback when a display disappears, reconnect restoration |
| Leave apps alone / recover windows | Exclusion list, include again, float-all, disable and emergency recovery shortcut |
| Every workspace action as a URL | Typed `fanzyzones://` commands, CLI and local D-Bus dispatch |
| New file-manager split | `Ctrl+Alt+N` launches a new Dolphin window beside the current space |
| Guided setup | Introduction, KDE compatibility and shortcut guidance, arrange all displays or keep floating, panel appearance |
| Settings / support | Shortcut editor, gap and animation settings, integrated stable-release check, editable diagnostic report draft |

KDE adaptations: Meta replaces Command; Dolphin replaces Finder. The custom
title strip's arrow button and `Ctrl+Alt+F` toggle docking/floating. Native
KWin maximize buttons also toggle docking; hold Alt for ordinary maximize.
Apps drawing their own title bars get a workspace strip when floating because
KWin cannot identify arbitrary client-side buttons; those app-owned buttons
retain their behavior. Application-owned decorations cannot always be removed.
Fullscreen and explicit native maximization are left to KWin.
Attached dialogs follow their managed parent's visibility and display.
KDE does not need macOS Accessibility or Screen Recording permission prompts.
All features are available without a trial, license, or per-display window cap.

The first opt-in from the menu opens setup, which can be reopened from
**Workspace setup…**. **Check for updates…** compares the installed version
against the latest stable GitHub release; installation stays with the existing
package manager or release installer. **Report a problem…** prepares an editable
local draft with version and capability counts. It excludes window titles,
application names, paths and logs. Opening the GitHub draft is an explicit user
action; nothing is submitted automatically. The CLI equivalents are
`fanzyzones-kde check-updates` and `fanzyzones-kde diagnostics`.

## Native input module

The two-finger gesture and Meta+Shift pointer interceptors use KWin's private
input API. Build against the **same KWin version** running the desktop. The
module checks that version before registering its input filter. Status reports
`nativeInput: false` when the module is absent or cannot be used.

The Nix package includes the module. A NixOS/Home Manager configuration should
use `inputs.fanzyzones-kde.inputs.nixpkgs.follows = "nixpkgs"`, so both come from
the same package set. The CLI and keyboard interactions do not require the
module.

For a non-Nix installation, install your distribution's matching KWin development
package, Qt 6 Quick/QML development packages and CMake, then:

```sh
cmake -S native -B target/native -DCMAKE_INSTALL_PREFIX="$HOME/.local"
cmake --build target/native
cmake --install target/native
fanzyzones-kde install --reload
```

This places the module under `~/.local/share/fanzyzones-kde/native`, outside the
embedded resources that upgrades replace. The installer copies it into the
effect when no bundled module is present. With custom XDG paths, pass
`FANZYZONES_KDE_NATIVE_DIR` pointing to the directory containing `qmldir` and
`libfanzyzonesinput.so`. Rebuild the module after changing KWin versions.

KWin caches QML components and native modules. Log out and back in after
upgrading their code; reconfiguration updates settings but cannot reliably
replace cached component code inside the running compositor.

## Panel and tray

The default is an icon in the system tray. Its menu includes Back, Forward,
Desktop, Everything and saved-view buttons. The current view is highlighted;
Shift-click a slot to save it.

For controls directly in the panel, add the FanzyZones widget to the panel
itself and turn off **Icon only** in its settings. System trays reserve a single
icon-sized slot, so use the direct panel placement for the expanded strip.
Right-click the strip for the menu. A display selector controls which display
the buttons navigate.

## Commands and state

```sh
fanzyzones-kde workspace status
fanzyzones-kde workspace focus
fanzyzones-kde workspace back
fanzyzones-kde workspace save-view --display DP-1 --slot 3
fanzyzones-kde workspace load-view --display DP-1 --slot 3
fanzyzones-kde workspace dock --window '{window-uuid}' --target '{other-window-uuid}' --edge center
fanzyzones-kde handle-url 'fanzyzones://show-all?display=DP-1'
xdg-open 'fanzyzones://load-view?slot=3&display=DP-1'
fanzyzones-kde workspace float-all
fanzyzones-kde workspace disable
```

`workspace --help` lists all actions. URL parameters support `display`, `slot`,
`window`, `target`, `edge`, `group`, `app` and normalized `rect=x,y,width,height`.
The parser rejects unknown actions and malformed parameters. Commands return an
acknowledgment from KWin; a timeout is reported as failure. Animation may finish
after the command is acknowledged; `workspace status` exposes `animating`.

Settings use the existing XDG config directory. Workspace trees, cameras, saved
views and recovery geometry are stored in `workspace.json` in the application's
XDG data directory, with a `.bak` copy. Neither screenshots nor recordings are
stored. Window matching uses the existing identity, then an exact app/title/role
match, then a unique app/role match. Ambiguous reopened windows get a new
floating slot rather than being assigned arbitrarily.

## Validation

The integration harness runs a separate KWin with a private D-Bus session,
Wayland socket and XDG directories. Its input driver refuses ordinary desktop
sockets. Tests compare the effect's model with KWin's independent
`getWindowInfo` results, and send actual pointer and keyboard events to native
fixture windows.

Validated on KWin 6.6.6 / Qt 6.11.2, Wayland, two virtual 1280×800 displays:

- Real window geometry, border restoration, nested splits, tabs and minimized
  slot restoration.
- Nine views, focus, history, display independence and cross-display moves.
- Meta+Shift drag framing, Escape cancellation and global shortcuts.
- Two-finger events injected through KWin's actual pointer input pipeline:
  continuous scale, reversal, cancellation, unmodified application pass-through,
  bare Desktop pinch, best-fit settling and one history entry per gesture.
- Native KWin title-bar double-click, maximize-button docking, Alt-maximize and
  restore; floating workspace controls for windows without server decorations.
- Pointer seam resize, title/Desktop double-click, keyboard input reaching the
  native client, dwell tabs, tab selection, tear-off and group dragging.
- Modal visibility following tab switches.
- Real Dolphin split launch, application exclusion and inclusion, fullscreen
  return, and Plasma menu/panel rendering.
- XWayland docking, mixed Wayland/X11 tabs, focus, minimize/restore and floating.
- Guided setup completion, live release check and editable diagnostic draft.
- Display disable/re-enable and 125% fractional scale.
- Full compositor restart with reopened window identities, saved views and
  display assignments restored.
- Rust storage/URL/config tests and JavaScript model tests, including 1,200
  deterministic mixed edits that check unique window ownership after each edit.

Physical trackpad hardware still needs a hands-on check: the pinch integration
driver exercises the compositor pipeline, not libinput or a physical device.
Other decoration themes, real monitor hotplug and different GPUs are not covered
by these virtual-session results. Coverage is of the implemented interactions,
not every application or compositor configuration.

## Running the checks

```sh
cargo test
cargo clippy --all-targets -- -D warnings
node --test tests/workspace.test.mjs
```

Build `native/` against the installed KWin first. Start the isolated session in
one terminal (the process remains in the foreground):

```sh
test_root=$(mktemp -d /tmp/fanzyzones-parity.XXXXXX)
QML_BIN=/path/to/qt6/bin/qml \
FANZYZONES_NATIVE_DIR=/path/to/built/native/module \
FANZYZONES_TEST_INPUT_DIR=/path/to/test/module \
FANZYZONES_TEST_XWAYLAND=1 \
  bash tests/integration/session.sh "$test_root" "$PWD"
```

Run `bash tests/integration/build-input.sh` inside `nix develop` to build the
input driver. Outside Nix, provide Wayland's scanner and development package,
`pkg-config`, and `FAKE_INPUT_XML` pointing to Plasma's `fake-input.xml`.
Build the optional pinch/decoration driver with `nix build .#integration-input`
(using the host's nixpkgs input) or CMake in `tests/integration/native-driver`.
Its installed directory is `share/fanzyzones-test-input`. The module refuses to
load outside the private virtual test compositor and is never shipped as part
of the application. `QML_BIN` should also be available to checks which launch
additional fixtures.

In another terminal, pass the printed private directory to the checks:

```sh
python tests/integration/in-session.py "$test_root" python tests/integration/input-check.py "$test_root"
python tests/integration/in-session.py "$test_root" python tests/integration/dialog-check.py "$test_root"
python tests/integration/in-session.py "$test_root" python tests/integration/output-check.py "$test_root"
python tests/integration/in-session.py "$test_root" python tests/integration/application-check.py "$test_root"
python tests/integration/in-session.py "$test_root" python tests/integration/gesture-check.py "$test_root"
python tests/integration/in-session.py "$test_root" python tests/integration/frameless-check.py "$test_root"
python tests/integration/in-session.py "$test_root" python tests/integration/xwayland-check.py "$test_root"
```

`check.py` also tests disabling the effect, and should run last in a fresh
three-fixture session. For restart coverage, run `restart-check.py "$test_root"
save` through `in-session.py`, stop that private compositor, restart
`session.sh` with the same directory, then run `restart-check.py` without `save`.
`capture.py` captures the private compositor through KWin's screenshot API.
Pass an output name as its third argument while an animation is active:
KWin 6.6's combined-area screenshot uses only the first output's fullscreen
effect view, whereas `CaptureScreen` exercises each display's actual view.
