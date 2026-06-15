//! Read and rebind the FanzyZones global shortcuts. The shortcuts themselves
//! are registered (with default keys) and handled by the KWin script, under the
//! "kwin" KGlobalAccel component. Because the KWin script keeps those actions
//! alive, any process may rebind them via `setForeignShortcut` — so the in-app
//! editor works without a long-running daemon; it just shells out to these CLI
//! commands.

use anyhow::{Context, Result};

const KGA_SERVICE: &str = "org.kde.kglobalaccel";
const KWIN_COMPONENT: &str = "kwin";
const KWIN_COMPONENT_FRIENDLY: &str = "KWin";

const META: i32 = 0x1000_0000;
const CTRL: i32 = 0x0400_0000;
const ALT: i32 = 0x0800_0000;
const SHIFT: i32 = 0x0200_0000;
const KEY_1: i32 = 0x31;
const KEY_C: i32 = 0x43;
const KEY_SPACE: i32 = 0x20;
const KEY_LEFT: i32 = 0x0100_0012;
const KEY_RIGHT: i32 = 0x0100_0014;
const KEY_UP: i32 = 0x0100_0013;
const KEY_DOWN: i32 = 0x0100_0015;
const KEY_PGUP: i32 = 0x0100_0016;
const KEY_PGDOWN: i32 = 0x0100_0017;

pub struct ShortcutDef {
    pub id: &'static str,
    pub friendly: String,
    pub default_key: i32,
}

/// The FanzyZones shortcut actions in display order. `friendly` matches the
/// KWin-script ShortcutHandler name without the "FanzyZones: " prefix.
pub fn shortcut_defs() -> Vec<ShortcutDef> {
    let mut list = Vec::new();
    for n in 1..=9 {
        list.push(ShortcutDef {
            id: leak(format!("snap-zone-{n}")),
            friendly: format!("Snap window to zone {n}"),
            default_key: META | CTRL | (KEY_1 + n - 1),
        });
    }
    for n in 1..=9 {
        list.push(ShortcutDef {
            id: leak(format!("use-layout-{n}")),
            friendly: format!("Use layout {n}"),
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
    for (id, friendly, default_key) in fixed {
        list.push(ShortcutDef { id, friendly: (*friendly).to_string(), default_key: *default_key });
    }
    list
}

/// Force-apply default keys for any FanzyZones shortcut that is currently
/// unbound. Run after (re)installing so the KWin-script shortcuts bind their
/// defaults even when KGlobalAccel cached a stale empty state for the action.
/// Existing user bindings (non-empty) are left untouched.
pub async fn apply_default_bindings() -> Result<()> {
    let connection = zbus::Connection::session().await.context("connect to session bus")?;
    let current = read_shortcuts_conn(&connection).await.unwrap_or_default();
    for def in shortcut_defs() {
        let bound = current
            .iter()
            .find(|(id, _, _)| id == def.id)
            .map(|(_, _, seq)| !seq.is_empty())
            .unwrap_or(false);
        if !bound {
            let _ = set_foreign_conn(&connection, &def.friendly, &keycode_to_string(def.default_key)).await;
        }
    }
    Ok(())
}

fn leak(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

fn action_name(friendly: &str) -> String {
    format!("FanzyZones: {friendly}")
}

fn key_name(key: i32) -> Option<String> {
    Some(match key {
        0x30..=0x39 | 0x41..=0x5a => ((key as u8) as char).to_string(),
        KEY_SPACE => "Space".into(),
        KEY_LEFT => "Left".into(),
        KEY_RIGHT => "Right".into(),
        KEY_UP => "Up".into(),
        KEY_DOWN => "Down".into(),
        KEY_PGUP => "PgUp".into(),
        KEY_PGDOWN => "PgDown".into(),
        0x0100_0030..=0x0100_003b => format!("F{}", key - 0x0100_0030 + 1),
        _ => return None,
    })
}

/// Render a Qt keycode as a portable sequence string ("Meta+Ctrl+1").
pub fn keycode_to_string(key: i32) -> String {
    if key == 0 {
        return String::new();
    }
    let mut parts = Vec::new();
    if key & META != 0 { parts.push("Meta".to_string()); }
    if key & CTRL != 0 { parts.push("Ctrl".to_string()); }
    if key & ALT != 0 { parts.push("Alt".to_string()); }
    if key & SHIFT != 0 { parts.push("Shift".to_string()); }
    match key_name(key & 0x01FF_FFFF) {
        Some(name) => parts.push(name),
        None => return String::new(),
    }
    parts.join("+")
}

/// Parse a portable sequence string into a Qt keycode (order-independent).
pub fn string_to_keycode(sequence: &str) -> Option<i32> {
    let sequence = sequence.trim();
    if sequence.is_empty() {
        return None;
    }
    let mut mods = 0;
    let mut base: Option<i32> = None;
    for token in sequence.split('+') {
        match token.trim().to_ascii_lowercase().as_str() {
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
                    if c.is_ascii_alphanumeric() {
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

type ShortcutInfo = (String, String, String, String, String, String, Vec<i32>, Vec<i32>);

async fn read_shortcuts_conn(connection: &zbus::Connection) -> Result<Vec<(String, String, String)>> {
    let reply = connection
        .call_method(
            Some(KGA_SERVICE),
            "/component/kwin",
            Some("org.kde.kglobalaccel.Component"),
            "allShortcutInfos",
            &(),
        )
        .await
        .context("query KGlobalAccel shortcuts")?;
    let infos: Vec<ShortcutInfo> = reply.body().deserialize().unwrap_or_default();

    let mut out = Vec::new();
    for def in shortcut_defs() {
        let name = action_name(&def.friendly);
        let key = infos
            .iter()
            .find(|info| info.0 == name)
            .and_then(|info| info.6.first().copied())
            .unwrap_or(0);
        out.push((def.id.to_string(), def.friendly.clone(), keycode_to_string(key)));
    }
    Ok(out)
}

async fn set_foreign_conn(connection: &zbus::Connection, friendly: &str, sequence: &str) -> Result<()> {
    let name = action_name(friendly);
    let action_id = vec![
        KWIN_COMPONENT.to_string(),
        name.clone(),
        KWIN_COMPONENT_FRIENDLY.to_string(),
        name,
    ];
    let keys: Vec<i32> = string_to_keycode(sequence).into_iter().collect();
    connection
        .call_method(
            Some(KGA_SERVICE),
            "/kglobalaccel",
            Some("org.kde.KGlobalAccel"),
            "setForeignShortcut",
            &(action_id, keys),
        )
        .await
        .context("setForeignShortcut")?;
    Ok(())
}

/// Read the current bindings for all FanzyZones shortcuts as
/// (id, friendly, sequence-string).
pub async fn read_shortcuts() -> Result<Vec<(String, String, String)>> {
    let connection = zbus::Connection::session().await.context("connect to session bus")?;
    read_shortcuts_conn(&connection).await
}

/// Rebind one FanzyZones shortcut (by id) to the given sequence string. An empty
/// sequence unbinds it.
pub async fn set_foreign_shortcut(id: &str, sequence: &str) -> Result<()> {
    let friendly = shortcut_defs()
        .into_iter()
        .find(|d| d.id == id)
        .map(|d| d.friendly)
        .with_context(|| format!("unknown shortcut id {id}"))?;
    let connection = zbus::Connection::session().await.context("connect to session bus")?;
    set_foreign_conn(&connection, &friendly, sequence).await
}
