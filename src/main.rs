mod config;
mod kwin;
mod layout;
mod tray;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use config::Settings;
use ksni::TrayMethods;
use kwin::KwinController;
use std::env;
use std::path::{Path, PathBuf};
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
            let path = config::save(&settings)?;
            ProcessCommand::new("xdg-open")
                .arg(&path)
                .spawn()
                .with_context(|| format!("open {}", path.display()))?;
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
                controller
                    .invoke_shortcut(&format!("FanzyZones: Activate layout {}", index + 1))
                    .await?;
                Ok(Some(settings))
            } else {
                anyhow::bail!("layout {} does not exist", index + 1);
            }
        }
        TrayMessage::SnapZone(index) => {
            controller
                .invoke_shortcut(&format!(
                    "FanzyZones: Move active window to zone {}",
                    index + 1
                ))
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
