mod config;
mod kwin;
mod layout;
mod resources;
mod shortcuts;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use config::{Settings, SnapMode};
pub(crate) use kwin::KwinController;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::process::Command as ProcessCommand;

const FANZY_DBUS_SERVICE: &str = "com.benwbooth.FanzyZones";
const FANZY_DBUS_PATH: &str = "/com/benwbooth/FanzyZones";
const FANZY_DBUS_INTERFACE: &str = "com.benwbooth.FanzyZones";
const FANZY_PLASMOID_ID: &str = "com.benwbooth.fanzyzones";

#[derive(Debug, Parser)]
#[command(author, version, about)]
struct Args {
    #[command(subcommand)]
    command: Option<CliCommand>,
}

#[derive(Debug, Subcommand)]
enum CliCommand {
    /// Install or upgrade the Plasma applet, DBus activation service, and KWin script.
    Install {
        /// Ask KWin to reload after writing config.
        #[arg(long)]
        reload: bool,
    },
    /// Install or upgrade only the Plasma applet and DBus activation service.
    InstallPlasmoid,
    /// Print applet state JSON for debugging.
    StateJson {
        /// Also run the idempotent KWin setup path before printing state.
        #[arg(long)]
        sync: bool,
    },
    /// Invoke a visual-menu action payload and print updated applet state JSON.
    InvokeAction {
        /// JSON payload such as {"action":"setLayout","layout":1}.
        payload: String,
    },
    /// Print the FanzyZones shortcuts as JSON [{id, friendly, sequence}].
    Shortcuts,
    /// Rebind a FanzyZones shortcut by id to a key sequence (empty unbinds it).
    SetShortcut {
        id: String,
        sequence: String,
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

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let args = Args::parse();
    match args.command {
        Some(command) => run_cli(command),
        None => {
            use clap::CommandFactory;
            Args::command().print_help().ok();
            Ok(())
        }
    }
}

fn run_cli(command: CliCommand) -> Result<()> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("create Tokio runtime")?
        .block_on(run_cli_async(command))
}

async fn run_cli_async(command: CliCommand) -> Result<()> {
    match command {
        CliCommand::Install { reload } => {
            install_plasma_integration().await?;
            let settings = load_and_save_settings()?;
            KwinController::from_environment()?
                .sync(&settings, reload)
                .await?;
            if reload {
                shortcuts::apply_default_bindings().await?;
            }
            Ok(())
        }
        CliCommand::InstallPlasmoid => install_plasma_integration().await,
        CliCommand::StateJson { sync } => {
            println!("{}", applet_state_json(sync).await?);
            Ok(())
        }
        CliCommand::InvokeAction { payload } => {
            println!("{}", invoke_action_payload(&payload).await?);
            Ok(())
        }
        CliCommand::Shortcuts => {
            let list: Vec<_> = shortcuts::read_shortcuts()
                .await?
                .into_iter()
                .map(|(id, friendly, sequence)| {
                    serde_json::json!({ "id": id, "friendly": friendly, "sequence": sequence })
                })
                .collect();
            println!("{}", serde_json::to_string(&list)?);
            Ok(())
        }
        CliCommand::SetShortcut { id, sequence } => {
            shortcuts::set_foreign_shortcut(&id, &sequence).await?;
            Ok(())
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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppletState {
    settings: Settings,
    status: String,
    tray_icon_source: String,
    service: &'static str,
    path: &'static str,
    interface: &'static str,
}

async fn applet_state_json(sync: bool) -> Result<String> {
    let settings = load_and_save_settings()?;
    let mut status = "KWin integration ready".to_string();

    if sync {
        status = match KwinController::from_environment() {
            Ok(controller) => match controller.sync(&settings, true).await {
                Ok(()) => "KWin integration ready".to_string(),
                Err(err) => format!("Error: {err:#}"),
            },
            Err(err) => format!("Error: {err:#}"),
        };
    }

    current_applet_state_json(status)
}

async fn invoke_action_payload(payload: &str) -> Result<String> {
    if payload.contains("\"action\":\"debugPlacement\"") {
        log_placement_debug(payload);
        return current_applet_state_json("KWin integration ready");
    }

    let request = parse_visual_menu_payload(payload)?;
    let mut settings = load_and_save_settings()?;
    let controller = KwinController::from_environment()?;
    let should_quit = handle_visual_menu_action(request.action, &controller, &mut settings).await?;
    let status = if should_quit {
        "FanzyZones backend stopped".to_string()
    } else {
        "KWin integration ready".to_string()
    };
    let state = current_applet_state_json(status)?;

    if should_quit {
        tokio::spawn(async {
            tokio::time::sleep(std::time::Duration::from_millis(75)).await;
            std::process::exit(0);
        });
    }

    Ok(state)
}

fn current_applet_state_json(status: impl Into<String>) -> Result<String> {
    let settings = load_and_save_settings()?;
    serde_json::to_string(&AppletState {
        settings,
        status: status.into(),
        tray_icon_source: tray_icon_source_url(),
        service: FANZY_DBUS_SERVICE,
        path: FANZY_DBUS_PATH,
        interface: FANZY_DBUS_INTERFACE,
    })
    .context("serialize applet state")
}

async fn install_plasma_integration() -> Result<()> {
    // A self-contained binary carries the plasmoid/KWin-script/icons embedded;
    // unpack them so the resolvers (and kpackagetool) have real directories.
    if resources::is_self_contained() {
        resources::extract_all()?;
    }
    // The icon theme is cosmetic; never let it abort the functional install.
    if let Err(err) = install_icon_theme() {
        eprintln!("fanzyzones-kde: skipping icon theme install: {err:#}");
    }
    install_cli_wrapper()?;
    remove_daemon_service().await;
    install_plasmoid_package().await?;
    install_system_tray_item().await?;
    Ok(())
}

/// Install a small wrapper that runs the fanzyzones-kde CLI with the right
/// environment. The Plasma applet shells out to this for its actions, so no
/// background daemon is needed.
fn install_cli_wrapper() -> Result<PathBuf> {
    let wrapper_dir = xdg_data_home()?.join("fanzyzones-kde");
    fs::create_dir_all(&wrapper_dir)
        .with_context(|| format!("create {}", wrapper_dir.display()))?;
    let wrapper = wrapper_dir.join("fanzyzones-kde");
    let exe = env::current_exe().context("resolve current fanzyzones-kde executable")?;
    let mut script = String::from("#!/bin/sh\n");
    for (key, value) in activation_environment()? {
        script.push_str("export ");
        script.push_str(key);
        script.push('=');
        script.push_str(&shell_quote(&value.to_string_lossy()));
        script.push('\n');
    }
    script.push_str("exec ");
    script.push_str(&shell_quote(&exe.to_string_lossy()));
    script.push_str(" \"$@\"\n");
    fs::write(&wrapper, script).with_context(|| format!("write {}", wrapper.display()))?;
    let mut permissions = fs::metadata(&wrapper)
        .with_context(|| format!("stat {}", wrapper.display()))?
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&wrapper, permissions)
        .with_context(|| format!("mark {} executable", wrapper.display()))?;
    Ok(wrapper)
}

/// Remove the legacy background daemon: stop/disable the user service and delete
/// its unit, DBus activation file, and wrapper.
async fn remove_daemon_service() {
    let _ = run_process("systemctl", &["--user", "stop", "fanzyzones-kde.service"]).await;
    let _ = run_process("systemctl", &["--user", "disable", "fanzyzones-kde.service"]).await;
    if let Ok(data_home) = xdg_data_home() {
        let _ = fs::remove_file(
            data_home
                .join("dbus-1/services")
                .join(format!("{FANZY_DBUS_SERVICE}.service")),
        );
        let _ = fs::remove_file(data_home.join("fanzyzones-kde/fanzyzones-kde-dbus-service"));
    }
    if let Ok(config_home) = xdg_config_home() {
        let _ = fs::remove_file(config_home.join("systemd/user/fanzyzones-kde.service"));
    }
    let _ = run_process("systemctl", &["--user", "daemon-reload"]).await;
}

fn install_icon_theme() -> Result<()> {
    let Some(source) = icon_theme_dir_path() else {
        return Ok(());
    };
    let target = xdg_data_home()?.join("icons");
    copy_dir_all(&source, &target).with_context(|| {
        format!(
            "install FanzyZones icons from {} to {}",
            source.display(),
            target.display()
        )
    })?;
    Ok(())
}

fn activation_environment() -> Result<Vec<(&'static str, PathBuf)>> {
    let mut values = Vec::new();
    let controller = KwinController::from_environment()?;
    values.push((
        "FANZYZONES_KDE_KWIN_SCRIPT_DIR",
        controller.script_dir().to_path_buf(),
    ));
    if let Some(path) = icon_theme_dir_path() {
        values.push(("FANZYZONES_KDE_ICON_THEME_DIR", path));
    }
    if let Some(path) = tray_icon_source_path() {
        values.push(("FANZYZONES_KDE_TRAY_ICON_SOURCE", path));
    }
    if let Ok(path) = plasmoid_package_path() {
        values.push(("FANZYZONES_KDE_PLASMOID_DIR", path));
    }
    Ok(values)
}

async fn install_plasmoid_package() -> Result<()> {
    let package = plasmoid_package_path()?;
    let package_arg = package.to_string_lossy().into_owned();
    let upgrade = run_process(
        "kpackagetool6",
        &["--type", "Plasma/Applet", "--upgrade", &package_arg],
    )
    .await;
    if upgrade.is_ok() {
        return Ok(());
    }

    let install = run_process(
        "kpackagetool6",
        &["--type", "Plasma/Applet", "--install", &package_arg],
    )
    .await;
    match install {
        Ok(()) => Ok(()),
        Err(install_err) => Err(install_err)
            .with_context(|| format!("upgrade attempt also failed: {:#}", upgrade.unwrap_err())),
    }
}

async fn install_system_tray_item() -> Result<()> {
    let config_path = plasma_applets_config_path()?;
    if !config_path.exists() {
        return Ok(());
    }

    let content = fs::read_to_string(&config_path)
        .with_context(|| format!("read {}", config_path.display()))?;
    let groups = system_tray_general_groups(&content);
    for group in groups {
        for key in ["extraItems", "knownItems"] {
            let current = kconfig_value(&content, &group, key).unwrap_or_default();
            let updated = append_csv_item(&current, FANZY_PLASMOID_ID);
            if updated != current {
                kwriteconfig(&group, key, &updated).await?;
            }
        }
    }

    reload_plasmashell_config().await;
    Ok(())
}

fn plasma_applets_config_path() -> Result<PathBuf> {
    if let Some(config_home) = env::var_os("XDG_CONFIG_HOME") {
        return Ok(PathBuf::from(config_home).join("plasma-org.kde.plasma.desktop-appletsrc"));
    }
    let home = env::var_os("HOME")
        .map(PathBuf::from)
        .context("resolve HOME for Plasma config path")?;
    Ok(home
        .join(".config")
        .join("plasma-org.kde.plasma.desktop-appletsrc"))
}

fn system_tray_general_groups(content: &str) -> Vec<Vec<String>> {
    let mut current_group = String::new();
    let mut groups = Vec::new();
    for line in content.lines().map(str::trim) {
        if let Some(group) = line
            .strip_prefix('[')
            .and_then(|line| line.strip_suffix(']'))
        {
            current_group = group.to_string();
            continue;
        }
        if line == "plugin=org.kde.plasma.systemtray" {
            let mut group = split_kconfig_group(&current_group);
            group.push("General".to_string());
            groups.push(group);
        }
    }
    groups
}

fn kconfig_value(content: &str, group: &[String], key: &str) -> Option<String> {
    let wanted_group = group.join("][");
    let mut in_group = false;
    for line in content.lines().map(str::trim) {
        if let Some(current_group) = line
            .strip_prefix('[')
            .and_then(|line| line.strip_suffix(']'))
        {
            in_group = current_group == wanted_group;
            continue;
        }
        if !in_group {
            continue;
        }
        let Some((line_key, value)) = line.split_once('=') else {
            continue;
        };
        if line_key == key {
            return Some(value.to_string());
        }
    }
    None
}

fn split_kconfig_group(group: &str) -> Vec<String> {
    group.split("][").map(ToString::to_string).collect()
}

fn append_csv_item(value: &str, item: &str) -> String {
    let mut items: Vec<String> = value
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(ToString::to_string)
        .collect();
    if !items.iter().any(|existing| existing == item) {
        items.push(item.to_string());
    }
    items.join(",")
}

async fn kwriteconfig(group: &[String], key: &str, value: &str) -> Result<()> {
    let mut args = vec![
        "--file".to_string(),
        "plasma-org.kde.plasma.desktop-appletsrc".to_string(),
    ];
    for part in group {
        args.push("--group".to_string());
        args.push(part.clone());
    }
    args.push("--key".to_string());
    args.push(key.to_string());
    args.push(value.to_string());

    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    run_process("kwriteconfig6", &arg_refs).await
}

async fn reload_plasmashell_config() {
    let script = r#"
        for (const panel of panels()) {
            for (const widget of panel.widgets()) {
                if (widget.type === 'org.kde.plasma.systemtray') {
                    widget.currentConfigGroup = ['General'];
                    const extra = String(widget.readConfig('extraItems') || '').split(',').filter(Boolean);
                    const known = String(widget.readConfig('knownItems') || '').split(',').filter(Boolean);
                    if (extra.indexOf('__FANZY_PLASMOID_ID__') < 0) extra.push('__FANZY_PLASMOID_ID__');
                    if (known.indexOf('__FANZY_PLASMOID_ID__') < 0) known.push('__FANZY_PLASMOID_ID__');
                    widget.writeConfig('extraItems', extra.join(','));
                    widget.writeConfig('knownItems', known.join(','));
                    widget.reloadConfig();
                }
            }
        }
    "#
    .replace("__FANZY_PLASMOID_ID__", FANZY_PLASMOID_ID);
    if run_process(
        "busctl",
        &[
            "--user",
            "call",
            "org.kde.plasmashell",
            "/PlasmaShell",
            "org.kde.PlasmaShell",
            "evaluateScript",
            "s",
            &script,
        ],
    )
    .await
    .is_ok()
    {
        return;
    }
    let _ = run_process(
        "qdbus6",
        &[
            "org.kde.plasmashell",
            "/PlasmaShell",
            "org.kde.PlasmaShell.evaluateScript",
            &script,
        ],
    )
    .await;
}

async fn run_process(program: &str, args: &[&str]) -> Result<()> {
    let output = ProcessCommand::new(program)
        .args(args)
        .output()
        .await
        .with_context(|| format!("run {program}"))?;
    if output.status.success() {
        return Ok(());
    }

    anyhow::bail!(
        "{} {:?} exited with {:?}\nstdout:\n{}\nstderr:\n{}",
        program,
        args,
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn xdg_data_home() -> Result<PathBuf> {
    if let Some(path) = env::var_os("XDG_DATA_HOME") {
        return Ok(PathBuf::from(path));
    }
    let home = env::var_os("HOME")
        .map(PathBuf::from)
        .context("resolve HOME for XDG data directory")?;
    Ok(home.join(".local/share"))
}

fn xdg_config_home() -> Result<PathBuf> {
    if let Some(path) = env::var_os("XDG_CONFIG_HOME") {
        return Ok(PathBuf::from(path));
    }
    let home = env::var_os("HOME")
        .map(PathBuf::from)
        .context("resolve HOME for XDG config directory")?;
    Ok(home.join(".config"))
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn copy_dir_all(source: &Path, target: &Path) -> Result<()> {
    if !source.exists() {
        return Ok(());
    }
    fs::create_dir_all(target).with_context(|| format!("create {}", target.display()))?;
    for entry in fs::read_dir(source).with_context(|| format!("read {}", source.display()))? {
        let entry = entry.with_context(|| format!("read entry in {}", source.display()))?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        if source_path.is_dir() {
            copy_dir_all(&source_path, &target_path)?;
        } else {
            // Sources can live in the read-only Nix store (mode 0444); fs::copy
            // preserves that, so a re-install would later fail to overwrite a
            // now-read-only destination. Remove any existing file first and make
            // the copy writable.
            let _ = fs::remove_file(&target_path);
            fs::copy(&source_path, &target_path).with_context(|| {
                format!(
                    "copy {} to {}",
                    source_path.display(),
                    target_path.display()
                )
            })?;
            if let Ok(metadata) = fs::metadata(&target_path) {
                let mut perms = metadata.permissions();
                perms.set_mode(0o644);
                let _ = fs::set_permissions(&target_path, perms);
            }
        }
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(tag = "action", rename_all = "camelCase")]
pub(crate) enum VisualMenuAction {
    SetLayout {
        layout: usize,
        /// When set, assign the layout to this display (screen name) instead of
        /// changing the global active layout.
        #[serde(default)]
        display: Option<String>,
    },
    /// Persist a layout change that the KWin script already applied at runtime
    /// (e.g. a keyboard switch), without restarting the script.
    SyncActiveLayout { layout: usize },
    /// Merge a partial settings object (changed fields only) from the config
    /// dialog, preserving layouts/active_layout/MRU.
    UpdateSettings { patch: serde_json::Value },
    Snap { layout: usize, zone: usize },
    SetSnapMode { mode: SnapMode },
    /// Persist a layout produced by the in-plasmoid editor: build it, upsert by
    /// id (or append), make it active, and resync KWin.
    SaveLayout { result: serde_json::Value },
    DeleteLayout { layout: usize },
    OpenSettings,
    RevealConfig,
    PreviousZone,
    NextZone,
    Sync,
    ReloadSettings,
    ReloadKwin,
    About,
    Quit,
}

#[derive(Debug)]
pub(crate) struct VisualMenuActionRequest {
    pub(crate) action: VisualMenuAction,
    /// Parsed from the payload's `closeMenu` flag; consumed by the tests and the
    /// plasmoid's payload contract rather than the backend binary itself.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) close_menu: bool,
}

fn placement_debug_enabled() -> bool {
    env::var_os("FANZYZONES_KDE_DEBUG_PLACEMENT").is_some()
}

pub(crate) fn log_placement_debug(message: impl AsRef<str>) {
    if !placement_debug_enabled() {
        return;
    }

    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    if let Ok(mut file) = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("/tmp/fanzyzones-kde.log")
    {
        let _ = writeln!(file, "[{millis}] {}", message.as_ref());
    }
}

pub(crate) async fn handle_visual_menu_action(
    action: VisualMenuAction,
    controller: &KwinController,
    settings: &mut Settings,
) -> Result<bool> {
    match action {
        VisualMenuAction::SetLayout { layout, display } => {
            ensure_layout_exists(settings, layout)?;
            if let Some(display) = display {
                let id = settings.layouts[layout].id.clone();
                settings.display_layouts.insert(display, id);
            }
            // Always update the global active layout too: it drives the overlay/
            // auto-snap, the MRU ordering, and the fallback for unassigned
            // displays, so a per-display pick must not leave them out of sync.
            settings.active_layout = layout;
            config::save(settings)?;
            controller.write_settings(settings).await?;
            if controller.set_runtime_layout(layout).await.is_err() {
                controller.reload_kwin().await?;
            }
        }
        VisualMenuAction::SyncActiveLayout { layout } => {
            // The KWin script already switched its runtime layout; just persist
            // it so the tray menu and config reflect the change. No restart.
            ensure_layout_exists(settings, layout)?;
            settings.active_layout = layout;
            config::save(settings)?;
            controller.write_settings(settings).await?;
        }
        VisualMenuAction::UpdateSettings { patch } => {
            let mut current =
                serde_json::to_value(&*settings).context("serialize current settings")?;
            if let (Some(obj), Some(fields)) = (current.as_object_mut(), patch.as_object()) {
                for (key, value) in fields {
                    obj.insert(key.clone(), value.clone());
                }
            }
            let mut updated: Settings =
                serde_json::from_value(current).context("apply settings patch")?;
            updated.normalize();
            *settings = updated;
            config::save(settings)?;
            controller.write_settings(settings).await?;
            reload_runtime_settings_or_kwin(controller).await?;
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
        VisualMenuAction::SaveLayout { result } => {
            let layout = layout_from_editor(result)?;
            upsert_layout(settings, layout);
            config::save(settings)?;
            controller.write_settings(settings).await?;
            reload_runtime_settings_or_kwin(controller).await?;
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
        VisualMenuAction::RevealConfig => {
            open_config_dir().await?;
        }
        VisualMenuAction::PreviousZone => {
            controller
                .invoke_shortcut("FanzyZones: Move active window to previous zone")
                .await?;
        }
        VisualMenuAction::NextZone => {
            controller
                .invoke_shortcut("FanzyZones: Move active window to next zone")
                .await?;
        }
        VisualMenuAction::Sync => {
            controller.sync(settings, true).await?;
        }
        VisualMenuAction::ReloadSettings => {
            controller.write_settings(settings).await?;
            reload_runtime_settings_or_kwin(controller).await?;
        }
        VisualMenuAction::ReloadKwin => {
            controller.reload_kwin().await?;
        }
        VisualMenuAction::About => {
            open_url("https://github.com/benwbooth/fanzyzones-kde").await?;
        }
        VisualMenuAction::Quit => {
            return Ok(true);
        }
    }
    Ok(false)
}

async fn reload_runtime_settings_or_kwin(controller: &KwinController) -> Result<()> {
    if controller.reload_runtime_settings().await.is_err() {
        controller.reload_kwin().await?;
    }
    Ok(())
}

pub(crate) fn parse_visual_menu_payload(payload: &str) -> Result<VisualMenuActionRequest> {
    let value: serde_json::Value = serde_json::from_str(payload)
        .with_context(|| format!("parse visual menu action {}", payload))?;
    let close_menu = value
        .get("closeMenu")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true);
    let action = serde_json::from_value(value)
        .with_context(|| format!("decode visual menu action {}", payload))?;
    Ok(VisualMenuActionRequest { action, close_menu })
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

fn layout_from_editor(value: serde_json::Value) -> Result<crate::layout::Layout> {
    let name = value
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("My Layout")
        .trim()
        .to_string();
    let name = if name.is_empty() {
        "My Layout".to_string()
    } else {
        name
    };
    let id = value
        .get("id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            let millis = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or_default();
            format!("custom.{millis}")
        });
    let zones_in = value
        .get("zones")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let mut zones = Vec::new();
    for (i, z) in zones_in.iter().enumerate() {
        let f = |k: &str| z.get(k).and_then(|v| v.as_f64()).unwrap_or(0.0);
        zones.push(crate::layout::Zone::new(
            i,
            format!("Zone {}", i + 1),
            f("x"),
            f("y"),
            f("width"),
            f("height"),
        ));
    }
    anyhow::ensure!(!zones.is_empty(), "layout editor returned no zones");
    Ok(crate::layout::Layout {
        id,
        name,
        is_built_in: false,
        padding: 0,
        zones,
    })
}

/// Replace the layout with the same id, or append it, then make it active.
fn upsert_layout(settings: &mut Settings, layout: crate::layout::Layout) {
    let id = layout.id.clone();
    if let Some(slot) = settings.layouts.iter_mut().find(|l| l.id == id) {
        *slot = layout;
    } else {
        settings.layouts.push(layout);
    }
    if let Some(idx) = settings.layouts.iter().position(|l| l.id == id) {
        settings.active_layout = idx;
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

async fn open_config_dir() -> Result<()> {
    let path = config::settings_path()?;
    let dir = path
        .parent()
        .with_context(|| format!("resolve parent for {}", path.display()))?;
    fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
    ProcessCommand::new("xdg-open")
        .arg(dir)
        .spawn()
        .with_context(|| format!("open {}", dir.display()))?;
    Ok(())
}

async fn open_url(url: &str) -> Result<()> {
    ProcessCommand::new("xdg-open")
        .arg(url)
        .spawn()
        .with_context(|| format!("open {url}"))?;
    Ok(())
}

fn plasmoid_package_path() -> Result<PathBuf> {
    let candidates = [
        env::var_os("FANZYZONES_KDE_PLASMOID_DIR").map(PathBuf::from),
        Some(resources::resource_root().join("plasma-applet")),
        env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(Path::to_path_buf))
            .map(|bin| {
                bin.join("../share/plasma/plasmoids")
                    .join(FANZY_PLASMOID_ID)
            }),
        env::current_dir().ok().map(|dir| dir.join("plasma-applet")),
    ];

    candidates
        .into_iter()
        .flatten()
        .find(|path| path.join("metadata.json").exists())
        .with_context(|| "locate FanzyZones Plasma applet package")
}

pub(crate) fn tray_icon_source_url() -> String {
    tray_icon_source_path()
        .map(|path| format!("file://{}", path.to_string_lossy()))
        .unwrap_or_default()
}

fn tray_icon_source_path() -> Option<PathBuf> {
    let candidates = [
        env::var_os("FANZYZONES_KDE_TRAY_ICON_SOURCE").map(PathBuf::from),
        env::var_os("FANZYZONES_KDE_ICON_THEME_DIR")
            .map(PathBuf::from)
            .map(|dir| dir.join("hicolor/scalable/status/fanzyzones-kde.svg")),
        Some(resources::resource_root().join("icons/hicolor/scalable/status/fanzyzones-kde.svg")),
        env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(Path::to_path_buf))
            .map(|bin| bin.join("../share/icons/hicolor/scalable/status/fanzyzones-kde.svg")),
        env::current_dir()
            .ok()
            .map(|dir| dir.join("resources/icons/hicolor/scalable/status/fanzyzones-kde.svg")),
    ];

    candidates.into_iter().flatten().find(|path| path.exists())
}

fn icon_theme_dir_path() -> Option<PathBuf> {
    let candidates = [
        env::var_os("FANZYZONES_KDE_ICON_THEME_DIR").map(PathBuf::from),
        Some(resources::resource_root().join("icons")),
        env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(Path::to_path_buf))
            .map(|bin| bin.join("../share/icons")),
        env::current_dir()
            .ok()
            .map(|dir| dir.join("resources/icons")),
    ];

    candidates.into_iter().flatten().find(|path| path.exists())
}

pub(crate) fn load_and_save_settings() -> Result<Settings> {
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
    fn parses_visual_menu_payload_close_menu_flag() {
        let request = parse_visual_menu_payload(
            "{\"action\":\"setLayout\",\"layout\":3,\"closeMenu\":false}",
        )
        .unwrap();

        assert!(!request.close_menu);
        match request.action {
            VisualMenuAction::SetLayout { layout, .. } => assert_eq!(layout, 3),
            other => panic!("unexpected action {other:?}"),
        }
    }

    #[test]
    fn visual_menu_payload_closes_by_default() {
        let request = parse_visual_menu_payload("{\"action\":\"setLayout\",\"layout\":1}").unwrap();

        assert!(request.close_menu);
    }

    #[test]
    fn editor_result_upserts_and_activates_layout() {
        let mut settings = Settings::default();
        let before = settings.layouts.len();

        let layout = layout_from_editor(serde_json::json!({
            "name": "My Layout",
            "id": serde_json::Value::Null,
            "zones": [
                {"x": 0.0, "y": 0.0, "width": 0.5, "height": 1.0},
                {"x": 0.5, "y": 0.0, "width": 0.5, "height": 1.0},
            ],
        }))
        .unwrap();
        upsert_layout(&mut settings, layout);

        assert_eq!(settings.layouts.len(), before + 1);
        assert_eq!(settings.active_layout, before);
        assert!(!settings.layouts[before].is_built_in);
        assert_eq!(settings.layouts[before].name, "My Layout");
        assert_eq!(settings.layouts[before].zones.len(), 2);
    }

    #[test]
    fn deleting_active_custom_layout_falls_back_to_first_layout() {
        let mut settings = Settings::default();
        let mut custom = settings.layouts[0].clone();
        custom.id = "custom.test".into();
        custom.name = "Temporary".into();
        custom.is_built_in = false;
        settings.layouts.push(custom);
        settings.active_layout = settings.layouts.len() - 1;

        let index = settings.active_layout;
        delete_custom_layout(&mut settings, index).unwrap();

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
