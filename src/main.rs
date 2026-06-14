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
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitStatus;
use std::process::Stdio;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::process::Command as ProcessCommand;
use tokio::sync::mpsc::UnboundedSender;
use tokio::task::JoinHandle;
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
            run_visual_menu_blocking(&controller, None, "KWin integration ready")
                .await
                .map(|_| ())
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
    let (menu_sender, mut menu_receiver) = tokio::sync::mpsc::unbounded_channel();
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

    let mut running_menu: Option<RunningVisualMenu> = None;
    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                if let Some(mut menu) = running_menu.take() {
                    close_visual_menu(&mut menu).await;
                }
                handle.shutdown().await;
                break;
            }
            Some(message) = receiver.recv() => {
                match message {
                    TrayMessage::StartupSync => {
                        let _ = handle
                            .update(|tray: &mut FanzyTray| {
                                tray.status = pending_status(&message).into();
                            })
                            .await;
                        let status = handle_message(message, &controller).await;
                        let _ = handle
                            .update(|tray: &mut FanzyTray| match status {
                                Ok(HandleOutcome::Settings(settings)) => {
                                    tray.settings = settings;
                                    tray.status = success_status(&TrayMessage::StartupSync).into();
                                }
                                Ok(HandleOutcome::Quit) => {
                                    tray.status = "Quitting...".into();
                                }
                                Err(err) => {
                                    tray.status = format!("Error: {err:#}");
                                }
                            })
                            .await;
                    }
                    TrayMessage::OpenVisualMenu {
                        source,
                        x,
                        y,
                        status,
                    } => {
                        if let Some(menu) = running_menu.as_mut() {
                            close_visual_menu(menu).await;
                            let _ = handle
                                .update(|tray: &mut FanzyTray| {
                                    tray.status = "FanzyZones menu closed".into();
                                })
                                .await;
                            continue;
                        }

                        let anchor = (x >= 0 && y >= 0).then_some((x, y));
                        log_placement_debug(format!(
                            "tray {source} x={x} y={y} anchor={anchor:?}"
                        ));
                        let _ = handle
                            .update(|tray: &mut FanzyTray| {
                                tray.status = "Opening FanzyZones menu...".into();
                            })
                            .await;
                        match spawn_visual_menu(anchor, &status, menu_sender.clone()).await {
                            Ok(menu) => running_menu = Some(menu),
                            Err(err) => {
                                let _ = handle
                                    .update(|tray: &mut FanzyTray| {
                                        tray.status = format!("Error: {err:#}");
                                    })
                                    .await;
                            }
                        }
                    }
                }
            }
            Some(event) = menu_receiver.recv() => {
                match event {
                    VisualMenuEvent::Action { token, payload } => {
                        let Some(mut menu) = running_menu.take() else {
                            continue;
                        };
                        if token != menu.token {
                            running_menu = Some(menu);
                            continue;
                        }
                        close_visual_menu(&mut menu).await;
                        let menu_status = handle_visual_menu_payload(&payload, &controller).await;
                        let should_quit = matches!(menu_status, Ok(HandleOutcome::Quit));
                        let _ = handle
                            .update(|tray: &mut FanzyTray| match menu_status {
                                Ok(HandleOutcome::Settings(settings)) => {
                                    tray.settings = settings;
                                    tray.status = "FanzyZones menu closed".into();
                                }
                                Ok(HandleOutcome::Quit) => {
                                    tray.status = "Quitting...".into();
                                }
                                Err(err) => {
                                    tray.status = format!("Error: {err:#}");
                                }
                            })
                            .await;
                        if should_quit {
                            handle.shutdown().await;
                            break;
                        }
                    }
                    VisualMenuEvent::DebugPlacement { token, payload } => {
                        let Some(menu) = running_menu.as_ref() else {
                            continue;
                        };
                        if token == menu.token {
                            log_placement_debug(payload);
                        }
                    }
                    VisualMenuEvent::Finished(finished) => {
                        let Some(menu) = running_menu.take() else {
                            continue;
                        };
                        if finished.token != menu.token {
                            running_menu = Some(menu);
                            continue;
                        }
                        menu.action_task.abort();

                        let menu_status = handle_visual_menu_finished(
                            finished.result,
                            menu.closing,
                            &controller,
                        ).await;
                        let should_quit = matches!(menu_status, Ok(HandleOutcome::Quit));
                        let _ = handle
                            .update(|tray: &mut FanzyTray| match menu_status {
                                Ok(HandleOutcome::Settings(settings)) => {
                                    tray.settings = settings;
                                    tray.status = "FanzyZones menu closed".into();
                                }
                                Ok(HandleOutcome::Quit) => {
                                    tray.status = "Quitting...".into();
                                }
                                Err(err) => {
                                    tray.status = format!("Error: {err:#}");
                                }
                            })
                            .await;
                        if should_quit {
                            handle.shutdown().await;
                            break;
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

#[derive(Debug)]
enum HandleOutcome {
    Settings(Settings),
    Quit,
}

async fn handle_message(
    message: TrayMessage,
    controller: &KwinController,
) -> Result<HandleOutcome> {
    match message {
        TrayMessage::StartupSync => {
            let settings = load_and_save_settings()?;
            controller.sync(&settings, true).await?;
            Ok(HandleOutcome::Settings(settings))
        }
        TrayMessage::OpenVisualMenu { .. } => unreachable!("visual menu is handled in run_tray"),
    }
}

fn pending_status(message: &TrayMessage) -> &'static str {
    match message {
        TrayMessage::StartupSync => "Setting up KWin integration...",
        TrayMessage::OpenVisualMenu { .. } => "Opening FanzyZones menu...",
    }
}

fn success_status(message: &TrayMessage) -> &'static str {
    match message {
        TrayMessage::StartupSync => "KWin integration ready",
        TrayMessage::OpenVisualMenu { .. } => "FanzyZones menu closed",
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
struct RunningVisualMenu {
    token: String,
    pid: Option<u32>,
    closing: bool,
    action_task: JoinHandle<()>,
}

#[derive(Debug)]
struct VisualMenuFinished {
    token: String,
    result: Result<VisualMenuOutput>,
}

#[derive(Debug)]
enum VisualMenuEvent {
    Action { token: String, payload: String },
    DebugPlacement { token: String, payload: String },
    Finished(VisualMenuFinished),
}

#[derive(Debug)]
struct VisualMenuOutput {
    qml_path: PathBuf,
    status: ExitStatus,
    stdout: String,
    stderr: String,
}

async fn spawn_visual_menu(
    anchor: Option<(i32, i32)>,
    status: &str,
    menu_sender: UnboundedSender<VisualMenuEvent>,
) -> Result<RunningVisualMenu> {
    let settings = load_and_save_settings()?;
    let qml_path = layout_menu_qml_path()?;
    let settings_json = settings.compact_json()?;
    let token = visual_menu_token()?;
    let action_listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .context("bind visual menu action listener")?;
    let action_url = format!(
        "http://127.0.0.1:{}/{}",
        action_listener.local_addr()?.port(),
        token
    );
    log_placement_debug(format!(
        "spawn visual menu token={token} anchor={anchor:?} qml={}",
        qml_path.display()
    ));
    let mut command = visual_menu_command(&qml_path, settings_json, anchor, status);
    command.arg("--fanzyzones-action-url").arg(&action_url);
    let child = command
        .spawn()
        .with_context(|| format!("open visual menu {}", qml_path.display()))?;
    let pid = child.id();
    let finished_token = token.clone();
    let action_token = token.clone();
    let action_sender = menu_sender.clone();
    let action_task = tokio::spawn(async move {
        while let Ok(payload) = receive_visual_menu_request(&action_listener).await {
            if payload.contains("\"action\":\"debugPlacement\"") {
                let _ = action_sender.send(VisualMenuEvent::DebugPlacement {
                    token: action_token.clone(),
                    payload,
                });
                continue;
            }

            let _ = action_sender.send(VisualMenuEvent::Action {
                token: action_token,
                payload,
            });
            break;
        }
    });
    tokio::spawn(async move {
        let result = child
            .wait_with_output()
            .await
            .map(|output| VisualMenuOutput {
                qml_path,
                status: output.status,
                stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            })
            .map_err(anyhow::Error::from);
        let _ = menu_sender.send(VisualMenuEvent::Finished(VisualMenuFinished {
            token: finished_token,
            result,
        }));
    });
    Ok(RunningVisualMenu {
        token,
        pid,
        closing: false,
        action_task,
    })
}

async fn receive_visual_menu_request(listener: &TcpListener) -> Result<String> {
    let (mut stream, _) = listener
        .accept()
        .await
        .context("accept visual menu action")?;
    let mut request = Vec::new();
    let mut buffer = [0_u8; 4096];
    let mut header_end = None;
    let mut content_length = None;

    loop {
        let read = stream
            .read(&mut buffer)
            .await
            .context("read visual menu action")?;
        if read == 0 {
            break;
        }
        request.extend_from_slice(&buffer[..read]);
        if request.len() > 64 * 1024 {
            anyhow::bail!("visual menu action request too large");
        }

        if header_end.is_none() {
            header_end = request.windows(4).position(|window| window == b"\r\n\r\n");
            if let Some(end) = header_end {
                let headers = String::from_utf8_lossy(&request[..end]);
                content_length = headers.lines().find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().ok())
                        .flatten()
                });
            }
        }

        if let (Some(end), Some(length)) = (header_end, content_length) {
            let body_start = end + 4;
            if request.len() >= body_start + length {
                let body = String::from_utf8(request[body_start..body_start + length].to_vec())
                    .context("decode visual menu action")?;
                stream
                    .write_all(
                        b"HTTP/1.1 204 No Content\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                    )
                    .await
                    .ok();
                return Ok(body);
            }
        }
    }

    anyhow::bail!("visual menu action request ended before body")
}

async fn close_visual_menu(menu: &mut RunningVisualMenu) {
    menu.closing = true;
    menu.action_task.abort();
    if let Some(pid) = menu.pid {
        let _ = ProcessCommand::new("kill")
            .arg(pid.to_string())
            .status()
            .await;
    }
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

async fn handle_visual_menu_finished(
    result: Result<VisualMenuOutput>,
    was_closed_by_toggle: bool,
    controller: &KwinController,
) -> Result<HandleOutcome> {
    match result {
        Ok(output) => handle_visual_menu_output(output, was_closed_by_toggle, controller).await,
        Err(_err) if was_closed_by_toggle => Ok(HandleOutcome::Settings(load_and_save_settings()?)),
        Err(err) => Err(err),
    }
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

async fn handle_visual_menu_payload(
    payload: &str,
    controller: &KwinController,
) -> Result<HandleOutcome> {
    let action = serde_json::from_str(payload)
        .with_context(|| format!("parse visual menu action {}", payload))?;
    let mut settings = load_and_save_settings()?;
    if handle_visual_menu_action(action, controller, &mut settings).await? {
        return Ok(HandleOutcome::Quit);
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

    if env::var_os("QT_QPA_PLATFORM").is_none()
        && env::var_os("WAYLAND_DISPLAY").is_some()
        && env::var_os("DISPLAY").is_some()
    {
        command.env("QT_QPA_PLATFORM", "xcb");
    }

    command
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

fn log_placement_debug(message: impl AsRef<str>) {
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

fn visual_menu_token() -> Result<String> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("read system time")?
        .as_millis();
    Ok(format!("menu-{millis}"))
}

async fn handle_visual_menu_action(
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
