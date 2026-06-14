use crate::layout::{built_in_layouts, clamp_layout_index, Layout};
use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum SnapMode {
    Auto,
    #[default]
    Modifier,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum ModifierKey {
    Shift,
    Control,
    Alt,
    Meta,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RgbColor {
    pub red: f64,
    pub green: f64,
    pub blue: f64,
}

impl Default for RgbColor {
    fn default() -> Self {
        Self {
            red: 0.18,
            green: 0.48,
            blue: 0.96,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Settings {
    #[serde(default = "settings_version")]
    pub version: u32,
    #[serde(default)]
    pub snap_mode: SnapMode,
    #[serde(default = "default_modifiers")]
    pub modifiers: Vec<ModifierKey>,
    #[serde(default)]
    pub active_layout: usize,
    #[serde(default = "default_gap")]
    pub gap: i32,
    #[serde(default = "default_outer_padding")]
    pub outer_padding: i32,
    #[serde(default = "default_true")]
    pub enable_zone_overlay: bool,
    #[serde(default = "default_true")]
    pub enable_zone_selector: bool,
    #[serde(default)]
    pub enable_edge_snapping: bool,
    #[serde(default = "default_true")]
    pub remember_window_geometries: bool,
    #[serde(default = "default_true")]
    pub keyboard_shortcuts_enabled: bool,
    #[serde(default)]
    pub highlight_color: RgbColor,
    #[serde(default = "default_overlay_opacity")]
    pub overlay_opacity: f64,
    #[serde(default = "default_true")]
    pub show_zone_numbers: bool,
    #[serde(default)]
    pub track_layout_per_screen: bool,
    #[serde(default)]
    pub track_layout_per_desktop: bool,
    #[serde(default)]
    pub auto_snap_new_windows: bool,
    #[serde(default = "default_true")]
    pub dynamic_workspaces: bool,
    #[serde(default)]
    pub keep_empty_middle_desktops: bool,
    #[serde(default = "default_true")]
    pub macsimize_fullscreen: bool,
    #[serde(default = "default_true")]
    pub macsimize_maximized: bool,
    #[serde(default)]
    pub macsimize_move_to_last_desktop: bool,
    #[serde(default = "default_true")]
    pub macsimize_exclusive_desktops: bool,
    #[serde(default = "default_skip_classes")]
    pub skipped_window_classes: Vec<String>,
    #[serde(default)]
    pub debug: bool,
    #[serde(default = "built_in_layouts")]
    pub layouts: Vec<Layout>,
}

fn settings_version() -> u32 {
    1
}

fn default_true() -> bool {
    true
}

fn default_gap() -> i32 {
    0
}

fn default_outer_padding() -> i32 {
    0
}

fn default_skip_classes() -> Vec<String> {
    [
        "krunner",
        "ksmserver",
        "ksmserver-logout-greeter",
        "ksplashqml",
        "kwin",
        "kwin_wayland",
        "org.kde.plasmashell",
        "org.kde.spectacle",
        "org.kde.yakuake",
        "plasmashell",
    ]
    .into_iter()
    .map(String::from)
    .collect()
}

fn default_modifiers() -> Vec<ModifierKey> {
    vec![ModifierKey::Shift]
}

fn default_overlay_opacity() -> f64 {
    0.35
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: settings_version(),
            snap_mode: SnapMode::Modifier,
            modifiers: default_modifiers(),
            active_layout: 0,
            gap: default_gap(),
            outer_padding: default_outer_padding(),
            enable_zone_overlay: true,
            enable_zone_selector: true,
            enable_edge_snapping: false,
            remember_window_geometries: true,
            keyboard_shortcuts_enabled: true,
            highlight_color: RgbColor::default(),
            overlay_opacity: default_overlay_opacity(),
            show_zone_numbers: true,
            track_layout_per_screen: false,
            track_layout_per_desktop: false,
            auto_snap_new_windows: false,
            dynamic_workspaces: true,
            keep_empty_middle_desktops: false,
            macsimize_fullscreen: true,
            macsimize_maximized: true,
            macsimize_move_to_last_desktop: false,
            macsimize_exclusive_desktops: true,
            skipped_window_classes: default_skip_classes(),
            debug: false,
            layouts: built_in_layouts(),
        }
    }
}

impl Settings {
    pub fn normalize(&mut self) {
        if self.layouts.is_empty() {
            self.layouts = built_in_layouts();
            self.active_layout = 0;
        }
        self.active_layout = clamp_layout_index(self.active_layout, &self.layouts);
        self.gap = self.gap.max(0);
        self.outer_padding = self.outer_padding.max(0);
        self.skipped_window_classes
            .retain(|class| !class.trim().is_empty());
        for class in &mut self.skipped_window_classes {
            *class = class.trim().to_lowercase();
        }
        self.skipped_window_classes.sort();
        self.skipped_window_classes.dedup();
        self.modifiers.sort();
        self.modifiers.dedup();
        self.overlay_opacity = self.overlay_opacity.clamp(0.05, 0.95);
    }

    pub fn compact_json(&self) -> Result<String> {
        serde_json::to_string(self).context("serialize settings for KWin")
    }

    pub fn pretty_json(&self) -> Result<String> {
        serde_json::to_string_pretty(self).context("serialize settings")
    }

    pub fn active_layout_name(&self) -> &str {
        self.layouts
            .get(self.active_layout)
            .map(|layout| layout.name.as_str())
            .unwrap_or("Unknown")
    }
}

pub fn project_dirs() -> Result<ProjectDirs> {
    ProjectDirs::from("", "", "fanzyzones-kde").context("resolve XDG project directories")
}

pub fn settings_path() -> Result<PathBuf> {
    Ok(project_dirs()?.config_dir().join("settings.json"))
}

pub fn load_or_default() -> Result<Settings> {
    let path = settings_path()?;
    if !path.exists() {
        let mut settings = Settings::default();
        settings.normalize();
        return Ok(settings);
    }

    let bytes = fs::read(&path).with_context(|| format!("read {}", path.display()))?;
    let mut settings: Settings =
        serde_json::from_slice(&bytes).with_context(|| format!("parse {}", path.display()))?;
    settings.normalize();
    Ok(settings)
}

pub fn save(settings: &Settings) -> Result<PathBuf> {
    let path = settings_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    let mut normalized = settings.clone();
    normalized.normalize();
    fs::write(&path, normalized.pretty_json()?)
        .with_context(|| format!("write {}", path.display()))?;
    Ok(path)
}

pub fn import_from(path: &PathBuf) -> Result<Settings> {
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    let mut settings: Settings =
        serde_json::from_slice(&bytes).with_context(|| format!("parse {}", path.display()))?;
    settings.normalize();
    Ok(settings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_settings_have_real_layouts() {
        let settings = Settings::default();
        assert_eq!(settings.layouts.len(), 7);
        assert_eq!(settings.active_layout_name(), "Two Panes");
        assert_eq!(settings.gap, 0);
        assert!(settings.dynamic_workspaces);
        assert!(settings.macsimize_fullscreen);
    }

    #[test]
    fn normalize_rebuilds_missing_layouts_and_dedupes_skip_classes() {
        let mut settings = Settings {
            layouts: vec![],
            active_layout: 99,
            skipped_window_classes: vec![" Firefox ".into(), "firefox".into(), "".into()],
            ..Settings::default()
        };
        settings.normalize();
        assert_eq!(settings.active_layout, 0);
        assert_eq!(settings.layouts.len(), 7);
        assert_eq!(settings.skipped_window_classes, vec!["firefox"]);
    }
}
