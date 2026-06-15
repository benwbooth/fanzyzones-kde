//! Global shortcuts registered by the daemon under a dedicated "fanzyzones"
//! KGlobalAccel component, so they can be rebound from the app's own settings
//! dialog (and appear grouped under "FanzyZones" in System Settings).
//!
//! On a key press KGlobalAccel emits `globalShortcutPressed`; we invoke the
//! matching keyless handler in the persistent KWin script by name, so the
//! action runs with full runtime state.

use anyhow::{Context, Result};
use futures_util::StreamExt;

use crate::config::Settings;

const KGA_SERVICE: &str = "org.kde.kglobalaccel";
const KGA_PATH: &str = "/kglobalaccel";
const KGA_IFACE: &str = "org.kde.KGlobalAccel";
const COMPONENT: &str = "fanzyzones";
const COMPONENT_FRIENDLY: &str = "FanzyZones";
const COMPONENT_PATH: &str = "/component/fanzyzones";
const COMPONENT_IFACE: &str = "org.kde.kglobalaccel.Component";

const META: i32 = 0x1000_0000;
const CTRL: i32 = 0x0400_0000;
const ALT: i32 = 0x0800_0000;
const SHIFT: i32 = 0x0200_0000;
const KEY_1: i32 = 0x31;
const KEY_SPACE: i32 = 0x20;
const KEY_C: i32 = 0x43;
const KEY_LEFT: i32 = 0x0100_0012;
const KEY_RIGHT: i32 = 0x0100_0014;
const KEY_UP: i32 = 0x0100_0013;
const KEY_DOWN: i32 = 0x0100_0015;
const KEY_PGUP: i32 = 0x0100_0016;
const KEY_PGDOWN: i32 = 0x0100_0017;

/// SetShortcut flags: SetPresent (2) | NoAutoloading (4).
const SET_FLAGS: u32 = 6;

pub struct ShortcutDef {
    pub id: String,
    pub friendly: String,
    pub default_key: i32,
}

/// The full set of FanzyZones shortcut actions with their default keys.
pub fn shortcut_defs() -> Vec<ShortcutDef> {
    let mut list = Vec::new();
    for n in 1..=9 {
        list.push(ShortcutDef {
            id: format!("snap-zone-{n}"),
            friendly: format!("Snap window to zone {n}"),
            default_key: META | CTRL | (KEY_1 + n - 1),
        });
    }
    for n in 1..=9 {
        list.push(ShortcutDef {
            id: format!("use-layout-{n}"),
            friendly: format!("Switch to layout {n}"),
            default_key: META | SHIFT | (KEY_1 + n - 1),
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
        list.push(ShortcutDef {
            id: (*id).to_string(),
            friendly: (*friendly).to_string(),
            default_key: *key,
        });
    }
    list
}

/// The effective key for an action: a user override if set, else the default.
fn effective_key(def: &ShortcutDef, settings: &Settings) -> i32 {
    settings
        .shortcut_overrides
        .get(&def.id)
        .and_then(|seq| string_to_keycode(seq))
        .unwrap_or(def.default_key)
}

fn key_name(key: i32) -> Option<String> {
    let name = match key {
        0x30..=0x39 => ((key as u8) as char).to_string(),
        0x41..=0x5a => ((key as u8) as char).to_string(),
        KEY_SPACE => "Space".to_string(),
        KEY_LEFT => "Left".to_string(),
        KEY_RIGHT => "Right".to_string(),
        KEY_UP => "Up".to_string(),
        KEY_DOWN => "Down".to_string(),
        KEY_PGUP => "PgUp".to_string(),
        KEY_PGDOWN => "PgDown".to_string(),
        0x0100_0030..=0x0100_003b => format!("F{}", key - 0x0100_0030 + 1),
        _ => return None,
    };
    Some(name)
}

/// Render a Qt keycode as a portable sequence string ("Meta+Ctrl+1").
pub fn keycode_to_string(key: i32) -> String {
    if key == 0 {
        return String::new();
    }
    let mut parts = Vec::new();
    if key & META != 0 {
        parts.push("Meta".to_string());
    }
    if key & CTRL != 0 {
        parts.push("Ctrl".to_string());
    }
    if key & ALT != 0 {
        parts.push("Alt".to_string());
    }
    if key & SHIFT != 0 {
        parts.push("Shift".to_string());
    }
    match key_name(key & 0x01FF_FFFF) {
        Some(name) => parts.push(name),
        None => return String::new(),
    }
    parts.join("+")
}

/// Parse a portable sequence string ("Meta+Ctrl+1") into a Qt keycode.
/// Order-independent. Returns None for an empty/unparseable string.
pub fn string_to_keycode(sequence: &str) -> Option<i32> {
    let sequence = sequence.trim();
    if sequence.is_empty() {
        return None;
    }
    let mut mods = 0;
    let mut base: Option<i32> = None;
    for token in sequence.split('+') {
        let token = token.trim();
        match token.to_ascii_lowercase().as_str() {
            "meta" | "super" => mods |= META,
            "ctrl" | "control" => mods |= CTRL,
            "alt" => mods |= ALT,
            "shift" => mods |= SHIFT,
            "space" => base = Some(KEY_SPACE),
            "left" => base = Some(KEY_LEFT),
            "right" => base = Some(KEY_RIGHT),
            "up" => base = Some(KEY_UP),
            "down" => base = Some(KEY_DOWN),
            "pgup" | "pageup" | "page up" => base = Some(KEY_PGUP),
            "pgdown" | "pagedown" | "page down" => base = Some(KEY_PGDOWN),
            other => {
                if other.len() == 1 {
                    let c = other.chars().next().unwrap().to_ascii_uppercase();
                    if c.is_ascii_digit() || c.is_ascii_alphabetic() {
                        base = Some(c as i32);
                    }
                } else if let Some(num) = other.strip_prefix('f') {
                    if let Ok(n) = num.parse::<i32>() {
                        if (1..=12).contains(&n) {
                            base = Some(0x0100_0030 + n - 1);
                        }
                    }
                }
            }
        }
    }
    base.map(|key| key | mods)
}

/// Effective shortcuts as (id, friendly, sequence-string), for the settings UI.
pub fn effective_shortcuts(settings: &Settings) -> Vec<(String, String, String)> {
    shortcut_defs()
        .into_iter()
        .map(|def| {
            let key = effective_key(&def, settings);
            (def.id, def.friendly, keycode_to_string(key))
        })
        .collect()
}

async fn register_one(
    connection: &zbus::Connection,
    id: &str,
    friendly: &str,
    key: i32,
) -> Result<()> {
    let action_id = vec![
        COMPONENT.to_string(),
        id.to_string(),
        COMPONENT_FRIENDLY.to_string(),
        friendly.to_string(),
    ];
    connection
        .call_method(Some(KGA_SERVICE), KGA_PATH, Some(KGA_IFACE), "doRegister", &(action_id.clone(),))
        .await
        .with_context(|| format!("doRegister {id}"))?;
    let keys = if key == 0 { Vec::new() } else { vec![key] };
    connection
        .call_method(
            Some(KGA_SERVICE),
            KGA_PATH,
            Some(KGA_IFACE),
            "setShortcut",
            &(action_id, keys, SET_FLAGS),
        )
        .await
        .with_context(|| format!("setShortcut {id}"))?;
    Ok(())
}

/// Apply a single shortcut binding at runtime (used by the settings dialog).
pub async fn apply_shortcut(connection: &zbus::Connection, id: &str, key: i32) -> Result<()> {
    let friendly = shortcut_defs()
        .into_iter()
        .find(|d| d.id == id)
        .map(|d| d.friendly)
        .unwrap_or_else(|| id.to_string());
    register_one(connection, id, &friendly, key).await
}

/// Register all FanzyZones global shortcuts (honouring user overrides) and
/// forward presses to the KWin script. Runs until the connection drops.
pub async fn register_and_listen(connection: zbus::Connection) -> Result<()> {
    let settings = crate::config::load_or_default().unwrap_or_default();
    let defs = shortcut_defs();
    for def in &defs {
        let key = effective_key(def, &settings);
        if let Err(error) = register_one(&connection, &def.id, &def.friendly, key).await {
            tracing::warn!(%error, id = %def.id, "failed to register shortcut");
        }
    }
    tracing::info!(count = defs.len(), "registered FanzyZones global shortcuts");

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
        if !defs.iter().any(|d| d.id == action) {
            continue;
        }
        if let Err(error) = crate::run_global_shortcut(&action).await {
            tracing::warn!(%error, %action, "failed to dispatch shortcut");
        }
    }
    Ok(())
}
