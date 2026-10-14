#!/usr/bin/env bash
# Run only on a private D-Bus session. All configuration and windows are isolated.
set -euo pipefail
test_root=${1:?pass a temporary directory}
repo_root=${2:?pass the repository directory}
case "$test_root" in
    /tmp/fanzyzones-parity.*) ;;
    *) echo 'Use a private /tmp/fanzyzones-parity.XXXXXX directory' >&2; exit 1 ;;
esac
export XDG_CONFIG_HOME="$test_root/config"
export XDG_DATA_HOME="$test_root/data"
export XDG_STATE_HOME="$test_root/state"
export XDG_CACHE_HOME="$test_root/cache"
export XDG_RUNTIME_DIR="$test_root/runtime"
export WAYLAND_DISPLAY="fanzyzones-test-${test_root##*.}"
mkdir -p "$XDG_RUNTIME_DIR"
chmod 700 "$XDG_RUNTIME_DIR"
if [ "${FANZYZONES_TEST_BUS:-}" != 1 ]; then
    # The bus must inherit the private XDG paths so activation also stays here.
    exec dbus-run-session -- env FANZYZONES_TEST_BUS=1 bash "$0" "$test_root" "$repo_root"
fi
printf '%s\n' "$test_root"
export QT_QUICK_CONTROLS_STYLE=Basic
export QT_LOGGING_RULES='qt.qpa.wayland.textinput=false'
export QT_DISABLE_DISK_CACHE=1
export QT_LOGGING_TO_CONSOLE=1
export QT_NO_XDG_DESKTOP_PORTAL=1
export KWIN_WAYLAND_NO_PERMISSION_CHECKS=1
export KWIN_SCREENSHOT_NO_PERMISSION_CHECKS=1
export FANZYZONES_KDE_KWIN_SCRIPT_DIR="$repo_root/kwin-script"
export FANZYZONES_KDE_EFFECT_DIR="$repo_root/kwin-effect"
mkdir -p "$XDG_CONFIG_HOME" "$XDG_DATA_HOME/kwin/effects" "$XDG_DATA_HOME/kwin/scripts"
chmod -R u+w "$XDG_DATA_HOME/kwin"
rm -rf "$XDG_DATA_HOME/kwin/effects/fanzyzones_workspace" "$XDG_DATA_HOME/kwin/scripts/fanzyzones_kde"
cp -R "$repo_root/kwin-effect" "$XDG_DATA_HOME/kwin/effects/fanzyzones_workspace"
if [ -n "${FANZYZONES_NATIVE_DIR:-}" ]; then
    cp -R --no-preserve=mode "$FANZYZONES_NATIVE_DIR" "$XDG_DATA_HOME/kwin/effects/fanzyzones_workspace/contents/ui/native"
fi
cp -R "$repo_root/kwin-script" "$XDG_DATA_HOME/kwin/scripts/fanzyzones_kde"
if [ ! -f "$XDG_CONFIG_HOME/kwinrc" ]; then
    cat > "$XDG_CONFIG_HOME/kwinrc" <<'EOF'
[Plugins]
fanzyzones_workspaceEnabled=true
fanzyzones_kdeEnabled=true
[Effect-fanzyzones_workspace]
settingsJson={"nested_enabled":true,"nested_animation_ms":100,"nested_excluded_apps":[]}
[Script-fanzyzones_kde]
settingsJson={"nested_enabled":true}
[Compositing]
Backend=OpenGL
EOF
fi
mkdir -p "$XDG_CONFIG_HOME/fanzyzones-kde"
if [ ! -f "$XDG_CONFIG_HOME/fanzyzones-kde/settings.json" ]; then
    printf '%s\n' '{"nested_enabled":true,"nested_animation_ms":100}' > "$XDG_CONFIG_HOME/fanzyzones-kde/settings.json"
fi
printf '%s' "$DBUS_SESSION_BUS_ADDRESS" > "$test_root/bus-address"
printf '%s' "$WAYLAND_DISPLAY" > "$test_root/socket"
"$repo_root/target/debug/fanzyzones-kde" serve-workspace > "$test_root/service.log" 2>&1 &
service_pid=$!
trap 'kill "$service_pid" 2>/dev/null || true' EXIT
"${KWIN_BIN:-kwin_wayland}" --virtual --width 1280 --height 800 --output-count 2 \
    --socket "$WAYLAND_DISPLAY" --no-lockscreen --no-kactivities > "$test_root/kwin.log" 2>&1 &
kwin_pid=$!
printf '%s' "$kwin_pid" > "$test_root/kwin-pid"
trap 'kill "$kwin_pid" "$service_pid" 2>/dev/null || true' EXIT
if [ -n "${QML_BIN:-}" ]; then
    for attempt in $(seq 1 50); do
        [ -S "$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY" ] && break
        sleep 0.1
    done
    for label in Alpha Beta Gamma; do
        QT_QPA_PLATFORM=wayland "$QML_BIN" \
            "$repo_root/tests/integration/Fixture.qml" -- "$label" > "$test_root/$label.log" 2>&1 &
    done
fi
wait "$kwin_pid"
