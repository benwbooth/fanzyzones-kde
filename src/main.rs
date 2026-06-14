mod config;
mod kwin;
mod layout;
mod tray;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use config::{Settings, SnapMode};
use ksni::TrayMethods;
use kwin::KwinController;
use serde::Deserialize;
use std::env;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::process::Command as ProcessCommand;
use tray::{FanzyTray, TrayMessage};

#[derive(Debug, Parser)]
#[command(author, version, about)]
struct Args {
    #[command(subcommand)]
    command: Option<CliCommand>,
}

#[derive(Debug, Subcommand)]
enum CliCommand {
    /// Run the KDE tray app.
    Tray,
    /// Open the FanzyZones visual layout menu.
    VisualMenu,
    /// Install or upgrade the bundled KWin script, write settings, and enable it.
    Install {
        /// Ask KWin to reload after writing config.
        #[arg(long)]
        reload: bool,
    },
    /// Write the current JSON settings into KWin's script config.
    WriteConfig,
    /// Ask KWin to reconfigure.
    ReloadKwin,
    /// Print the settings file path.
    ConfigPath,
    /// Print the current settings JSON.
    PrintConfig,
    /// Replace settings with defaults.
    ResetConfig,
    /// Import settings and layouts from a JSON file.
    ImportConfig {
        path: PathBuf,
        /// Also sync the imported settings to KWin.
        #[arg(long)]
        sync: bool,
    },
    /// Select the active layout by zero-based index or layout id/name.
    SetLayout {
        layout: String,
        /// Also sync the updated settings to KWin.
        #[arg(long)]
        sync: bool,
    },
    /// Snap the focused window to a one-based zone in the active layout.
    SnapZone {
        zone: usize,
        /// Use this layout instead of the active layout.
        #[arg(long)]
        layout: Option<String>,
    },
    /// Disable the KWin script.
    Disable,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let args = Args::parse();
    match args.command.unwrap_or(CliCommand::Tray) {
        CliCommand::Tray => run_tray().await,
        CliCommand::VisualMenu => {
            let controller = KwinController::from_environment()?;
            run_visual_menu(&controller, None).await.map(|_| ())
        }
        CliCommand::Install { reload } => {
            let settings = load_and_save_settings()?;
            KwinController::from_environment()?
                .sync(&settings, reload)
                .await
        }
        CliCommand::WriteConfig => {
            let settings = load_and_save_settings()?;
            KwinController::from_environment()?
                .write_settings(&settings)
                .await
        }
        CliCommand::ReloadKwin => KwinController::from_environment()?.reload_kwin().await,
        CliCommand::ConfigPath => {
            println!("{}", config::settings_path()?.display());
            Ok(())
        }
        CliCommand::PrintConfig => {
            let settings = load_and_save_settings()?;
            println!("{}", settings.pretty_json()?);
            Ok(())
        }
        CliCommand::ResetConfig => {
            let settings = Settings::default();
            let path = config::save(&settings)?;
            println!("Wrote {}", path.display());
            Ok(())
        }
        CliCommand::ImportConfig { path, sync } => {
            let settings = config::import_from(&path)?;
            let saved = config::save(&settings)?;
            println!("Imported {}", saved.display());
            if sync {
                KwinController::from_environment()?
                    .sync(&settings, true)
                    .await?;
            }
            Ok(())
        }
        CliCommand::SetLayout { layout, sync } => {
            let mut settings = load_and_save_settings()?;
            settings.active_layout = resolve_layout(&settings, &layout)?;
            let saved = config::save(&settings)?;
            println!(
                "Selected {} in {}",
                settings.active_layout_name(),
                saved.display()
            );
            if sync {
                KwinController::from_environment()?
                    .sync(&settings, true)
                    .await?;
            }
            Ok(())
        }
        CliCommand::SnapZone { zone, layout } => {
            anyhow::ensure!(zone > 0, "zone must be 1 or greater");
            let settings = load_and_save_settings()?;
            let layout_index = if let Some(layout) = layout {
                resolve_layout(&settings, &layout)?
            } else {
                settings.active_layout
            };
            let zone_index = zone - 1;
            ensure_zone_exists(&settings, layout_index, zone_index)?;
            KwinController::from_environment()?
                .snap_active_window_to_layout_zone(&settings, layout_index, zone_index)
                .await
        }
        CliCommand::Disable => KwinController::from_environment()?.disable_script().await,
    }
}

async fn run_tray() -> Result<()> {
    let settings = load_and_save_settings()?;
    let controller = KwinController::from_environment()?;
    let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
    let startup_sender = sender.clone();
    let tray = FanzyTray {
        settings,
        status: format!(
            "Setting up KWin integration from {}...",
            controller.script_dir().display()
        ),
        icon_theme_path: icon_theme_dir(),
        sender,
    };
    let handle = tray
        .assume_sni_available(true)
        .spawn()
        .await
        .context("spawn KDE tray item")?;

    let _ = startup_sender.send(TrayMessage::StartupSync);

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                handle.shutdown().await;
                break;
            }
            Some(message) = receiver.recv() => {
                if matches!(message, TrayMessage::Quit) {
                    handle.shutdown().await;
                    break;
                }
                let pending_status = pending_status(&message);
                let success_status = success_status(&message);
                let _ = handle.update(|tray: &mut FanzyTray| {
                    tray.status = pending_status.into();
                }).await;
                let status = handle_message(message, &controller).await;
                let _ = handle.update(|tray: &mut FanzyTray| {
                    match status {
                        Ok(Some(settings)) => {
                            tray.settings = settings;
                            tray.status = success_status.into();
                        }
                        Ok(None) => {
                            tray.status = success_status.into();
                        }
                        Err(err) => {
                            tray.status = format!("Error: {err:#}");
                        }
                    }
                }).await;
            }
        }
    }

    Ok(())
}

async fn handle_message(
    message: TrayMessage,
    controller: &KwinController,
) -> Result<Option<Settings>> {
    match message {
        TrayMessage::StartupSync => {
            let settings = load_and_save_settings()?;
            controller.sync(&settings, true).await?;
            Ok(Some(settings))
        }
        TrayMessage::OpenVisualMenu { x, y } => {
            let anchor = (x >= 0 && y >= 0).then_some((x, y));
            let settings = run_visual_menu(controller, anchor).await?;
            Ok(Some(settings))
        }
        TrayMessage::Sync => {
            let settings = load_and_save_settings()?;
            controller.sync(&settings, true).await?;
            Ok(Some(settings))
        }
        TrayMessage::ReloadKwin => {
            controller.reload_kwin().await?;
            Ok(None)
        }
        TrayMessage::OpenSettings => {
            let settings = load_and_save_settings()?;
            open_settings_file(&settings).await?;
            Ok(Some(settings))
        }
        TrayMessage::ReloadSettings => {
            let settings = load_and_save_settings()?;
            controller.write_settings(&settings).await?;
            controller.reload_kwin().await?;
            Ok(Some(settings))
        }
        TrayMessage::SetLayout(index) => {
            let mut settings = load_and_save_settings()?;
            if index < settings.layouts.len() {
                settings.active_layout = index;
                config::save(&settings)?;
                controller.write_settings(&settings).await?;
                controller.set_runtime_layout(index).await?;
                Ok(Some(settings))
            } else {
                anyhow::bail!("layout {} does not exist", index + 1);
            }
        }
        TrayMessage::SnapZone(index) => {
            let settings = load_and_save_settings()?;
            controller
                .snap_active_window_to_layout_zone(&settings, settings.active_layout, index)
                .await?;
            Ok(None)
        }
        TrayMessage::NextZone => {
            controller
                .invoke_shortcut("FanzyZones: Move active window to next zone")
                .await?;
            Ok(None)
        }
        TrayMessage::PreviousZone => {
            controller
                .invoke_shortcut("FanzyZones: Move active window to previous zone")
                .await?;
            Ok(None)
        }
        TrayMessage::Quit => Ok(None),
    }
}

fn pending_status(message: &TrayMessage) -> &'static str {
    match message {
        TrayMessage::StartupSync => "Setting up KWin integration...",
        TrayMessage::OpenVisualMenu { .. } => "Opening FanzyZones menu...",
        TrayMessage::Sync => "Installing KWin script...",
        TrayMessage::ReloadKwin => "Reloading KWin...",
        TrayMessage::OpenSettings => "Opening settings...",
        TrayMessage::ReloadSettings => "Reloading settings...",
        TrayMessage::SetLayout(_) => "Changing layout...",
        TrayMessage::SnapZone(_) => "Moving focused window...",
        TrayMessage::NextZone => "Moving focused window...",
        TrayMessage::PreviousZone => "Moving focused window...",
        TrayMessage::Quit => "Quitting...",
    }
}

fn success_status(message: &TrayMessage) -> &'static str {
    match message {
        TrayMessage::StartupSync => "KWin integration ready",
        TrayMessage::OpenVisualMenu { .. } => "FanzyZones menu closed",
        TrayMessage::Sync => "KWin script installed and enabled",
        TrayMessage::ReloadKwin => "KWin reloaded",
        TrayMessage::OpenSettings => "Settings opened",
        TrayMessage::ReloadSettings => "Settings synced to KWin",
        TrayMessage::SetLayout(_) => "Layout changed",
        TrayMessage::SnapZone(_) => "Window moved",
        TrayMessage::NextZone => "Window moved",
        TrayMessage::PreviousZone => "Window moved",
        TrayMessage::Quit => "Quitting...",
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "action", rename_all = "camelCase")]
enum VisualMenuAction {
    SetLayout { layout: usize },
    Snap { layout: usize, zone: usize },
    SetSnapMode { mode: SnapMode },
    CreateLayout,
    EditLayout { layout: usize },
    DeleteLayout { layout: usize },
    OpenSettings,
}

async fn run_visual_menu(
    controller: &KwinController,
    anchor: Option<(i32, i32)>,
) -> Result<Settings> {
    let settings = load_and_save_settings()?;
    let qml_path = layout_menu_qml_path()?;
    let settings_json = settings.compact_json()?;
    let output = ProcessCommand::new("qml")
        .arg(&qml_path)
        .arg("--")
        .arg(settings_json)
        .args(anchor.into_iter().flat_map(|(x, y)| {
            [
                "--fanzyzones-anchor".to_string(),
                x.to_string(),
                y.to_string(),
            ]
        }))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .with_context(|| format!("open visual menu {}", qml_path.display()))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !output.status.success() {
        anyhow::bail!(
            "visual menu exited with {:?}\nstdout:\n{}\nstderr:\n{}",
            output.status.code(),
            stdout,
            stderr
        );
    }

    let mut settings = load_and_save_settings()?;
    if let Some(action) = parse_visual_menu_action(&stdout, &stderr)? {
        handle_visual_menu_action(action, controller, &mut settings).await?;
    }
    Ok(settings)
}

async fn handle_visual_menu_action(
    action: VisualMenuAction,
    controller: &KwinController,
    settings: &mut Settings,
) -> Result<()> {
    match action {
        VisualMenuAction::SetLayout { layout } => {
            ensure_layout_exists(settings, layout)?;
            settings.active_layout = layout;
            config::save(settings)?;
            controller.write_settings(settings).await?;
            if controller.set_runtime_layout(layout).await.is_err() {
                controller.reload_kwin().await?;
            }
        }
        VisualMenuAction::Snap { layout, zone } => {
            ensure_zone_exists(settings, layout, zone)?;
            controller
                .snap_active_window_to_layout_zone(settings, layout, zone)
                .await?;
        }
        VisualMenuAction::SetSnapMode { mode } => {
            settings.snap_mode = mode;
            config::save(settings)?;
            controller.write_settings(settings).await?;
            reload_runtime_settings_or_kwin(controller).await?;
        }
        VisualMenuAction::CreateLayout => {
            create_custom_layout(settings)?;
            config::save(settings)?;
            controller.write_settings(settings).await?;
            reload_runtime_settings_or_kwin(controller).await?;
            open_settings_file(settings).await?;
        }
        VisualMenuAction::EditLayout { layout } => {
            ensure_layout_exists(settings, layout)?;
            open_settings_file(settings).await?;
        }
        VisualMenuAction::DeleteLayout { layout } => {
            delete_custom_layout(settings, layout)?;
            config::save(settings)?;
            controller.write_settings(settings).await?;
            reload_runtime_settings_or_kwin(controller).await?;
        }
        VisualMenuAction::OpenSettings => {
            open_settings_file(settings).await?;
        }
    }
    Ok(())
}

async fn reload_runtime_settings_or_kwin(controller: &KwinController) -> Result<()> {
    if controller.reload_runtime_settings().await.is_err() {
        controller.reload_kwin().await?;
    }
    Ok(())
}

fn parse_visual_menu_action(stdout: &str, stderr: &str) -> Result<Option<VisualMenuAction>> {
    const PREFIX: &str = "FANZYZONES_ACTION ";
    for line in stdout.lines().chain(stderr.lines()) {
        if let Some(offset) = line.find(PREFIX) {
            let payload = &line[offset + PREFIX.len()..];
            return serde_json::from_str(payload)
                .map(Some)
                .with_context(|| format!("parse visual menu action {}", payload));
        }
    }
    Ok(None)
}

fn ensure_layout_exists(settings: &Settings, layout: usize) -> Result<()> {
    if layout < settings.layouts.len() {
        Ok(())
    } else {
        anyhow::bail!("layout {} does not exist", layout + 1);
    }
}

fn ensure_zone_exists(settings: &Settings, layout: usize, zone: usize) -> Result<()> {
    ensure_layout_exists(settings, layout)?;
    if zone < settings.layouts[layout].zones.len() {
        Ok(())
    } else {
        anyhow::bail!(
            "zone {} does not exist in {}",
            zone + 1,
            settings.layouts[layout].name
        );
    }
}

fn create_custom_layout(settings: &mut Settings) -> Result<()> {
    ensure_layout_exists(settings, settings.active_layout)?;
    let mut layout = settings.layouts[settings.active_layout].clone();
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("read system time")?
        .as_millis();
    layout.id = format!("custom.{millis}");
    layout.name = next_custom_layout_name(settings);
    layout.is_built_in = false;
    settings.layouts.push(layout);
    settings.active_layout = settings.layouts.len() - 1;
    Ok(())
}

fn next_custom_layout_name(settings: &Settings) -> String {
    let base = "My Layout";
    if !settings.layouts.iter().any(|layout| layout.name == base) {
        return base.into();
    }

    let mut suffix = 2;
    loop {
        let candidate = format!("{base} {suffix}");
        if !settings
            .layouts
            .iter()
            .any(|layout| layout.name == candidate)
        {
            return candidate;
        }
        suffix += 1;
    }
}

fn delete_custom_layout(settings: &mut Settings, layout_index: usize) -> Result<()> {
    ensure_layout_exists(settings, layout_index)?;
    if settings.layouts[layout_index].is_built_in {
        anyhow::bail!("built-in layouts cannot be deleted");
    }

    settings.layouts.remove(layout_index);
    if settings.layouts.is_empty() {
        settings.layouts = layout::built_in_layouts();
    }
    if settings.active_layout == layout_index {
        settings.active_layout = 0;
    } else if settings.active_layout > layout_index {
        settings.active_layout -= 1;
    } else if settings.active_layout >= settings.layouts.len() {
        settings.active_layout = settings.layouts.len().saturating_sub(1);
    }
    Ok(())
}

async fn open_settings_file(settings: &Settings) -> Result<()> {
    let path = config::save(settings)?;
    ProcessCommand::new("xdg-open")
        .arg(&path)
        .spawn()
        .with_context(|| format!("open {}", path.display()))?;
    Ok(())
}

fn layout_menu_qml_path() -> Result<PathBuf> {
    let candidates = [
        env::var_os("FANZYZONES_KDE_LAYOUT_MENU_QML").map(PathBuf::from),
        env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(Path::to_path_buf))
            .map(|bin| bin.join("../share/fanzyzones-kde/qml/LayoutMenu.qml")),
        env::current_dir()
            .ok()
            .map(|dir| dir.join("resources/qml/LayoutMenu.qml")),
    ];

    candidates
        .into_iter()
        .flatten()
        .find(|path| path.exists())
        .with_context(|| "locate FanzyZones visual menu QML")
}

fn icon_theme_dir() -> String {
    let candidates = [
        env::var_os("FANZYZONES_KDE_ICON_THEME_DIR").map(PathBuf::from),
        env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(Path::to_path_buf))
            .map(|bin| bin.join("../share/icons")),
        env::current_dir()
            .ok()
            .map(|dir| dir.join("resources/icons")),
    ];

    candidates
        .into_iter()
        .flatten()
        .find(|path| path.exists())
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn load_and_save_settings() -> Result<Settings> {
    let settings = config::load_or_default()?;
    config::save(&settings)?;
    Ok(settings)
}

fn resolve_layout(settings: &Settings, input: &str) -> Result<usize> {
    if let Ok(index) = input.parse::<usize>() {
        if index < settings.layouts.len() {
            return Ok(index);
        }
    }

    let needle = input.to_lowercase();
    settings
        .layouts
        .iter()
        .position(|layout| {
            layout.id.to_lowercase() == needle || layout.name.to_lowercase() == needle
        })
        .with_context(|| format!("unknown layout '{}'", input))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_visual_menu_action_from_qml_output() {
        let action = parse_visual_menu_action(
            "",
            "qml: FANZYZONES_ACTION {\"action\":\"snap\",\"layout\":2,\"zone\":1}",
        )
        .unwrap()
        .unwrap();

        match action {
            VisualMenuAction::Snap { layout, zone } => {
                assert_eq!(layout, 2);
                assert_eq!(zone, 1);
            }
            other => panic!("unexpected action {other:?}"),
        }
    }

    #[test]
    fn creates_custom_layout_from_active_layout() {
        let mut settings = Settings {
            active_layout: 1,
            ..Settings::default()
        };

        create_custom_layout(&mut settings).unwrap();

        assert_eq!(settings.layouts.len(), 8);
        assert_eq!(settings.active_layout, 7);
        assert!(!settings.layouts[7].is_built_in);
        assert_eq!(settings.layouts[7].name, "My Layout");
        assert_eq!(settings.layouts[7].zones, settings.layouts[1].zones);
    }

    #[test]
    fn deleting_active_custom_layout_falls_back_to_first_layout() {
        let mut settings = Settings::default();
        create_custom_layout(&mut settings).unwrap();

        delete_custom_layout(&mut settings, 7).unwrap();

        assert_eq!(settings.layouts.len(), 7);
        assert_eq!(settings.active_layout, 0);
    }

    #[test]
    fn deleting_custom_layout_before_active_layout_shifts_active_index() {
        let mut settings = Settings {
            active_layout: 3,
            ..Settings::default()
        };
        let mut custom = settings.layouts[0].clone();
        custom.id = "custom.test".into();
        custom.name = "Temporary".into();
        custom.is_built_in = false;
        settings.layouts.insert(1, custom);

        delete_custom_layout(&mut settings, 1).unwrap();

        assert_eq!(settings.layouts.len(), 7);
        assert_eq!(settings.active_layout, 2);
    }
}
