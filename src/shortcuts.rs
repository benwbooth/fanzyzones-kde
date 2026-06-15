//! Global shortcuts registered by the daemon under a dedicated "fanzyzones"
//! KGlobalAccel component, so they appear grouped under "FanzyZones" in System
//! Settings → Shortcuts instead of buried in the KWin component.
//!
//! On a key press KGlobalAccel emits `globalShortcutPressed`; we forward the
//! corresponding action to the persistent KWin script via its pending-action
//! channel (kwinrc config + the "Process pending action" trigger), so the
//! script performs the window operation with full runtime state.

use anyhow::{Context, Result};
use futures_util::StreamExt;

const KGA_SERVICE: &str = "org.kde.kglobalaccel";
const KGA_PATH: &str = "/kglobalaccel";
const KGA_IFACE: &str = "org.kde.KGlobalAccel";
const COMPONENT: &str = "fanzyzones";
const COMPONENT_FRIENDLY: &str = "FanzyZones";
const COMPONENT_PATH: &str = "/component/fanzyzones";
const COMPONENT_IFACE: &str = "org.kde.kglobalaccel.Component";

// Qt keyboard modifier bits.
const META: i32 = 0x1000_0000;
const CTRL: i32 = 0x0400_0000;
const ALT: i32 = 0x0800_0000;
const SHIFT: i32 = 0x0200_0000;
// Qt key codes.
const KEY_1: i32 = 0x31;
const KEY_SPACE: i32 = 0x20;
const KEY_C: i32 = 0x43;
const KEY_LEFT: i32 = 0x0100_0012;
const KEY_RIGHT: i32 = 0x0100_0014;
const KEY_PGUP: i32 = 0x0100_0016;
const KEY_PGDOWN: i32 = 0x0100_0017;

/// SetShortcut flags: SetPresent (2) | NoAutoloading (4) — assign the given key
/// as the active shortcut and don't overwrite it from saved config.
const SET_FLAGS: u32 = 6;

struct Shortcut {
    id: String,
    friendly: String,
    key: i32,
}

fn build_shortcuts() -> Vec<Shortcut> {
    let mut list = Vec::new();
    for n in 1..=9 {
        list.push(Shortcut {
            id: format!("snap-zone-{n}"),
            friendly: format!("Snap window to zone {n}"),
            key: META | CTRL | (KEY_1 + n - 1),
        });
        list.push(Shortcut {
            id: format!("use-layout-{n}"),
            friendly: format!("Switch to layout {n}"),
            key: META | SHIFT | (KEY_1 + n - 1),
        });
    }
    let fixed: &[(&str, &str, i32)] = &[
        ("next-zone", "Snap window to next zone", CTRL | ALT | KEY_RIGHT),
        ("previous-zone", "Snap window to previous zone", CTRL | ALT | KEY_LEFT),
        ("next-layout", "Next layout", META | SHIFT | KEY_PGDOWN),
        ("previous-layout", "Previous layout", META | SHIFT | KEY_PGUP),
        ("snap-focused", "Snap focused window", META | SHIFT | KEY_SPACE),
        ("snap-all", "Snap all windows", META | KEY_SPACE),
        ("toggle-overlay", "Toggle zone overlay", CTRL | ALT | KEY_C),
    ];
    for (id, friendly, key) in fixed {
        list.push(Shortcut {
            id: (*id).to_string(),
            friendly: (*friendly).to_string(),
            key: *key,
        });
    }
    list
}

async fn register_one(connection: &zbus::Connection, shortcut: &Shortcut) -> Result<()> {
    let action_id = vec![
        COMPONENT.to_string(),
        shortcut.id.clone(),
        COMPONENT_FRIENDLY.to_string(),
        shortcut.friendly.clone(),
    ];
    connection
        .call_method(Some(KGA_SERVICE), KGA_PATH, Some(KGA_IFACE), "doRegister", &(action_id.clone(),))
        .await
        .with_context(|| format!("doRegister {}", shortcut.id))?;

    // KGlobalAccel identifies an action by its full 4-element actionId
    // [component, action, componentFriendly, actionFriendly]; setShortcut must
    // use the same id or it silently fails to match the action.
    let reply = connection
        .call_method(
            Some(KGA_SERVICE),
            KGA_PATH,
            Some(KGA_IFACE),
            "setShortcut",
            &(action_id, vec![shortcut.key], SET_FLAGS),
        )
        .await
        .with_context(|| format!("setShortcut {}", shortcut.id))?;
    let granted: Vec<i32> = reply.body().deserialize().unwrap_or_default();
    if granted.is_empty() {
        tracing::warn!(id = %shortcut.id, key = shortcut.key, "shortcut key not granted");
    }
    Ok(())
}

/// Register the FanzyZones global shortcuts and forward presses to the KWin
/// script. Runs forever; returns only on a fatal connection error.
pub async fn register_and_listen(connection: zbus::Connection) -> Result<()> {
    let shortcuts = build_shortcuts();
    for shortcut in &shortcuts {
        if let Err(error) = register_one(&connection, shortcut).await {
            tracing::warn!(%error, id = %shortcut.id, "failed to register shortcut");
        }
    }
    tracing::info!(count = shortcuts.len(), "registered FanzyZones global shortcuts");

    let proxy = zbus::Proxy::new(&connection, KGA_SERVICE, COMPONENT_PATH, COMPONENT_IFACE)
        .await
        .context("create FanzyZones shortcut component proxy")?;
    let mut presses = proxy
        .receive_signal("globalShortcutPressed")
        .await
        .context("subscribe to globalShortcutPressed")?;

    while let Some(signal) = presses.next().await {
        let (_component, action, _timestamp): (String, String, i64) =
            match signal.body().deserialize() {
                Ok(parsed) => parsed,
                Err(error) => {
                    tracing::warn!(%error, "could not decode globalShortcutPressed");
                    continue;
                }
            };
        if !shortcuts.iter().any(|s| s.id == action) {
            continue;
        }
        if let Err(error) = crate::run_global_shortcut(&action).await {
            tracing::warn!(%error, %action, "failed to dispatch shortcut");
        }
    }
    Ok(())
}
