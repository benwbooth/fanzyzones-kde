mod backend;
mod config;
mod kwin;
mod layout;
mod shortcuts;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use config::{Settings, SnapMode};
use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QString, QUrl};
pub(crate) use kwin::KwinController;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::ExitStatus;
use std::process::Stdio;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::process::Command as ProcessCommand;
use tokio::sync::Mutex;

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
    /// Run the DBus backend used by the Plasma applet.
    Daemon,
    /// Run the KDE tray app.
    Tray,
    /// Open the FanzyZones visual layout menu.
    VisualMenu,
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
    match args.command.unwrap_or(CliCommand::Daemon) {
        CliCommand::Tray => run_tray(),
        command => run_cli(command),
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
        CliCommand::Tray => unreachable!("tray is handled by the Qt event loop"),
        CliCommand::Daemon => run_dbus_daemon().await,
        CliCommand::VisualMenu => {
            let controller = KwinController::from_environment()?;
            match run_visual_menu_blocking(&controller, None, "KWin integration ready").await? {
                HandleOutcome::Settings(settings) => {
                    let _active_layout = settings.active_layout;
                    Ok(())
                }
                HandleOutcome::Quit => Ok(()),
            }
        }
        CliCommand::Install { reload } => {
            install_plasma_integration().await?;
            let settings = load_and_save_settings()?;
            KwinController::from_environment()?
                .sync(&settings, reload)
                .await
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

struct FanzyDbusService {
    setup_completed: Mutex<bool>,
}

impl FanzyDbusService {
    fn new() -> Self {
        Self {
            setup_completed: Mutex::new(false),
        }
    }
}

#[zbus::interface(name = "com.benwbooth.FanzyZones")]
impl FanzyDbusService {
    #[zbus(name = "State")]
    async fn state(&self) -> zbus::fdo::Result<String> {
        let mut setup_completed = self.setup_completed.lock().await;
        let run_setup = !*setup_completed;
        let state = applet_state_json(run_setup).await.map_err(fdo_error)?;
        if run_setup {
            *setup_completed = true;
        }
        Ok(state)
    }

    #[zbus(name = "Refresh")]
    async fn refresh(&self) -> zbus::fdo::Result<String> {
        let state = applet_state_json(true).await.map_err(fdo_error)?;
        *self.setup_completed.lock().await = true;
        Ok(state)
    }

    #[zbus(name = "InvokeAction")]
    async fn invoke_action(&self, payload: &str) -> zbus::fdo::Result<String> {
        invoke_action_payload(payload).await.map_err(fdo_error)
    }
}

fn fdo_error(err: anyhow::Error) -> zbus::fdo::Error {
    zbus::fdo::Error::Failed(format!("{err:#}"))
}

async fn run_dbus_daemon() -> Result<()> {
    tracing::info!("starting FanzyZones DBus backend");
    let connection = zbus::connection::Builder::session()
        .context("create FanzyZones DBus session builder")?
        .serve_at(FANZY_DBUS_PATH, FanzyDbusService::new())
        .context("serve FanzyZones DBus object")?
        .name(FANZY_DBUS_SERVICE)
        .context("request FanzyZones DBus service name")?
        .build()
        .await
        .context("connect FanzyZones DBus backend")?;

    // Register global shortcuts under a dedicated "FanzyZones" component and
    // forward presses to the KWin script. Uses this long-lived connection so
    // KGlobalAccel keeps the bindings active.
    let shortcut_connection = connection.clone();
    tokio::spawn(async move {
        if let Err(error) = shortcuts::register_and_listen(shortcut_connection).await {
            tracing::error!(%error, "FanzyZones shortcut listener stopped");
        }
    });

    std::future::pending::<()>().await;
    #[allow(unreachable_code)]
    Ok(())
}

fn run_tray() -> Result<()> {
    configure_qt_platform_environment();
    cxx_qt::init_crate!(fanzyzones_kde);

    let mut app = QGuiApplication::new();
    app.pin_mut()
        .set_application_name(&QString::from("FanzyZones KDE"));
    app.pin_mut()
        .set_application_version(&QString::from(env!("CARGO_PKG_VERSION")));
    QGuiApplication::set_desktop_file_name(&QString::from("fanzyzones-kde"));

    let mut engine = QQmlApplicationEngine::new();
    let _object_created_guard = engine.pin_mut().on_object_created(|_engine, object, url| {
        if object.is_null() {
            eprintln!(
                "FanzyZones tray QML did not create a root object: {}",
                String::from(url.to_local_file_or_default())
            );
        }
    });
    let _object_creation_failed_guard =
        engine.pin_mut().on_object_creation_failed(|_engine, url| {
            eprintln!(
                "FanzyZones tray QML object creation failed: {}",
                String::from(url.to_local_file_or_default())
            );
        });
    add_generated_qml_import_path(engine.pin_mut());
    let qml_path = tray_menu_qml_path()?;
    let qml_url = QUrl::from_local_file(&QString::from(qml_path.to_string_lossy().into_owned()));
    engine.pin_mut().load(&qml_url);
    let code = app.pin_mut().exec();
    anyhow::ensure!(code == 0, "Qt tray app exited with {code}");
    Ok(())
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
    install_icon_theme()?;
    install_dbus_activation_service()?;
    reload_dbus_activation_config().await;
    install_systemd_user_service().await?;
    install_plasmoid_package().await?;
    install_system_tray_item().await?;
    Ok(())
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

fn install_dbus_activation_service() -> Result<()> {
    let data_home = xdg_data_home()?;
    let service_dir = data_home.join("dbus-1/services");
    let wrapper_dir = data_home.join("fanzyzones-kde");
    fs::create_dir_all(&service_dir)
        .with_context(|| format!("create {}", service_dir.display()))?;
    fs::create_dir_all(&wrapper_dir)
        .with_context(|| format!("create {}", wrapper_dir.display()))?;

    let wrapper = wrapper_dir.join("fanzyzones-kde-dbus-service");
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
    script.push_str(" daemon \"$@\"\n");

    fs::write(&wrapper, script).with_context(|| format!("write {}", wrapper.display()))?;
    let mut permissions = fs::metadata(&wrapper)
        .with_context(|| format!("stat {}", wrapper.display()))?
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&wrapper, permissions)
        .with_context(|| format!("mark {} executable", wrapper.display()))?;

    let service_path = service_dir.join(format!("{FANZY_DBUS_SERVICE}.service"));
    let service = format!(
        "[D-BUS Service]\nName={FANZY_DBUS_SERVICE}\nExec={}\nSystemdService=fanzyzones-kde.service\n",
        wrapper.display()
    );
    fs::write(&service_path, service)
        .with_context(|| format!("write {}", service_path.display()))?;
    Ok(())
}

async fn reload_dbus_activation_config() {
    let _ = run_process(
        "busctl",
        &[
            "--user",
            "call",
            "org.freedesktop.DBus",
            "/org/freedesktop/DBus",
            "org.freedesktop.DBus",
            "ReloadConfig",
        ],
    )
    .await;
}

async fn install_systemd_user_service() -> Result<()> {
    if run_process("systemctl", &["--user", "--version"])
        .await
        .is_err()
    {
        return Ok(());
    }

    let config_home = xdg_config_home()?;
    let service_dir = config_home.join("systemd/user");
    fs::create_dir_all(&service_dir)
        .with_context(|| format!("create {}", service_dir.display()))?;

    let wrapper = xdg_data_home()?.join("fanzyzones-kde/fanzyzones-kde-dbus-service");
    let service_path = service_dir.join("fanzyzones-kde.service");
    let service = format!(
        "[Unit]\n\
         Description=FanzyZones KDE DBus backend\n\
         PartOf=graphical-session.target\n\
         After=graphical-session.target\n\
         \n\
         [Service]\n\
         Type=dbus\n\
         BusName={FANZY_DBUS_SERVICE}\n\
         ExecStart={}\n\
         Restart=on-failure\n\
         RestartSec=1\n\
         \n\
         [Install]\n\
         WantedBy=default.target\n",
        wrapper.display()
    );
    fs::write(&service_path, service)
        .with_context(|| format!("write {}", service_path.display()))?;

    run_process("systemctl", &["--user", "daemon-reload"]).await?;
    run_process("systemctl", &["--user", "enable", "fanzyzones-kde.service"]).await?;
    run_process(
        "systemctl",
        &["--user", "restart", "fanzyzones-kde.service"],
    )
    .await?;
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
    if let Ok(path) = layout_menu_qml_path() {
        values.push(("FANZYZONES_KDE_LAYOUT_MENU_QML", path));
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
            fs::copy(&source_path, &target_path).with_context(|| {
                format!(
                    "copy {} to {}",
                    source_path.display(),
                    target_path.display()
                )
            })?;
        }
    }
    Ok(())
}

#[derive(Debug)]
pub(crate) enum HandleOutcome {
    Settings(Settings),
    Quit,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "action", rename_all = "camelCase")]
pub(crate) enum VisualMenuAction {
    SetLayout { layout: usize },
    /// Persist a layout change that the KWin script already applied at runtime
    /// (e.g. a keyboard switch), without restarting the script.
    SyncActiveLayout { layout: usize },
    /// Merge a partial settings object (changed fields only) from the config
    /// dialog, preserving layouts/active_layout/MRU.
    UpdateSettings { patch: serde_json::Value },
    Snap { layout: usize, zone: usize },
    SetSnapMode { mode: SnapMode },
    CreateLayout,
    EditLayout { layout: usize },
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
    pub(crate) close_menu: bool,
}

#[derive(Debug)]
struct VisualMenuOutput {
    qml_path: PathBuf,
    status: ExitStatus,
    stdout: String,
    stderr: String,
}

async fn run_visual_menu_blocking(
    controller: &KwinController,
    anchor: Option<(i32, i32)>,
    status: &str,
) -> Result<HandleOutcome> {
    let settings = load_and_save_settings()?;
    let qml_path = layout_menu_qml_path()?;
    let settings_json = settings.compact_json()?;
    let output = visual_menu_command(&qml_path, settings_json, anchor, status)
        .output()
        .await
        .with_context(|| format!("open visual menu {}", qml_path.display()))?;

    handle_visual_menu_output(
        VisualMenuOutput {
            qml_path,
            status: output.status,
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        },
        false,
        controller,
    )
    .await
}

async fn handle_visual_menu_output(
    output: VisualMenuOutput,
    was_closed_by_toggle: bool,
    controller: &KwinController,
) -> Result<HandleOutcome> {
    log_visual_menu_debug_output(&output);
    if !output.status.success() {
        if was_closed_by_toggle {
            return Ok(HandleOutcome::Settings(load_and_save_settings()?));
        }
        anyhow::bail!(
            "visual menu {} exited with {:?}\nstdout:\n{}\nstderr:\n{}",
            output.qml_path.display(),
            output.status.code(),
            output.stdout,
            output.stderr
        );
    }

    let mut settings = load_and_save_settings()?;
    if let Some(action) = parse_visual_menu_action(&output.stdout, &output.stderr)? {
        if handle_visual_menu_action(action, controller, &mut settings).await? {
            return Ok(HandleOutcome::Quit);
        }
    }
    Ok(HandleOutcome::Settings(settings))
}

fn visual_menu_command(
    qml_path: &Path,
    settings_json: String,
    anchor: Option<(i32, i32)>,
    status: &str,
) -> ProcessCommand {
    let mut command = ProcessCommand::new("qml");
    command
        .arg(qml_path)
        .arg("--")
        .arg(settings_json)
        .arg("--fanzyzones-status")
        .arg(status)
        .args(anchor.into_iter().flat_map(|(x, y)| {
            [
                "--fanzyzones-anchor".to_string(),
                x.to_string(),
                y.to_string(),
            ]
        }))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if placement_debug_enabled() {
        command.arg("--fanzyzones-debug-placement");
    }

    configure_visual_menu_platform(&mut command);

    command
}

fn configure_visual_menu_platform(command: &mut ProcessCommand) {
    if env::var_os("QT_QPA_PLATFORM").is_none()
        && env::var_os("WAYLAND_DISPLAY").is_some()
        && env::var_os("DISPLAY").is_some()
    {
        command.env("QT_QPA_PLATFORM", "xcb");
    }
}

fn configure_qt_platform_environment() {
    if env::var_os("QT_QPA_PLATFORM").is_none()
        && env::var_os("WAYLAND_DISPLAY").is_some()
        && env::var_os("DISPLAY").is_some()
    {
        env::set_var("QT_QPA_PLATFORM", "xcb");
    }
}

fn placement_debug_enabled() -> bool {
    env::var_os("FANZYZONES_KDE_DEBUG_PLACEMENT").is_some()
}

fn log_visual_menu_debug_output(output: &VisualMenuOutput) {
    if !placement_debug_enabled() {
        return;
    }

    for line in output.stdout.lines().chain(output.stderr.lines()) {
        if line.contains("FANZYZONES_PLACEMENT") {
            log_placement_debug(line);
        }
    }
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

/// Dispatch a global shortcut (fired by KGlobalAccel under the "fanzyzones"
/// component) by invoking the matching keyless handler in the persistent KWin
/// script by name, so the action runs with full script state.
pub(crate) async fn run_global_shortcut(action: &str) -> Result<()> {
    let name = if let Some(n) = action.strip_prefix("snap-zone-") {
        format!("FanzyZones: Snap window to zone {n}")
    } else if let Some(n) = action.strip_prefix("use-layout-") {
        format!("FanzyZones: Use layout {n}")
    } else {
        match action {
            "next-zone" => "FanzyZones: Snap window to next zone".to_string(),
            "previous-zone" => "FanzyZones: Snap window to previous zone".to_string(),
            "next-layout" => "FanzyZones: Next layout".to_string(),
            "previous-layout" => "FanzyZones: Previous layout".to_string(),
            "snap-focused" => "FanzyZones: Snap focused window".to_string(),
            "snap-all" => "FanzyZones: Snap all windows".to_string(),
            "toggle-overlay" => "FanzyZones: Toggle zone overlay".to_string(),
            _ => {
                tracing::debug!(%action, "unknown global shortcut");
                return Ok(());
            }
        }
    };
    KwinController::from_environment()?
        .invoke_shortcut(&name)
        .await
}

pub(crate) async fn handle_visual_menu_action(
    action: VisualMenuAction,
    controller: &KwinController,
    settings: &mut Settings,
) -> Result<bool> {
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

fn parse_visual_menu_action(stdout: &str, stderr: &str) -> Result<Option<VisualMenuAction>> {
    const PREFIX: &str = "FANZYZONES_ACTION ";
    for line in stdout.lines().chain(stderr.lines()) {
        if let Some(offset) = line.find(PREFIX) {
            let payload = &line[offset + PREFIX.len()..];
            return parse_visual_menu_payload(payload).map(|request| Some(request.action));
        }
    }
    Ok(None)
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

fn tray_menu_qml_path() -> Result<PathBuf> {
    let layout_path = layout_menu_qml_path()?;
    let tray_path = layout_path.with_file_name("TrayMenu.qml");
    if tray_path.exists() {
        Ok(tray_path)
    } else {
        Ok(layout_path)
    }
}

fn plasmoid_package_path() -> Result<PathBuf> {
    let candidates = [
        env::var_os("FANZYZONES_KDE_PLASMOID_DIR").map(PathBuf::from),
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

fn add_generated_qml_import_path(mut engine: std::pin::Pin<&mut QQmlApplicationEngine>) {
    let path = PathBuf::from(env!("OUT_DIR")).join("qt-build-utils/qml_modules");
    if path.exists() {
        engine
            .as_mut()
            .add_import_path(&QString::from(path.to_string_lossy().into_owned()));
    }
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

pub(crate) fn tray_icon_theme_path() -> String {
    icon_theme_dir_path()
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn icon_theme_dir_path() -> Option<PathBuf> {
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
    fn parses_visual_menu_payload_close_menu_flag() {
        let request = parse_visual_menu_payload(
            "{\"action\":\"setLayout\",\"layout\":3,\"closeMenu\":false}",
        )
        .unwrap();

        assert!(!request.close_menu);
        match request.action {
            VisualMenuAction::SetLayout { layout } => assert_eq!(layout, 3),
            other => panic!("unexpected action {other:?}"),
        }
    }

    #[test]
    fn visual_menu_payload_closes_by_default() {
        let request = parse_visual_menu_payload("{\"action\":\"setLayout\",\"layout\":1}").unwrap();

        assert!(request.close_menu);
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
