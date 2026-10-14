//! IPC and durable storage for the nested workspace. KWin owns all live window
//! operations; this on-demand session service only saves JSON and handles commands.
use anyhow::{bail, Context, Result};
use clap::ValueEnum;
use serde_json::{json, Value};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::process::Command;

pub const EFFECT_ID: &str = "fanzyzones_workspace";
const MAX_STATE_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Action {
    Enable,
    Disable,
    Arrange,
    FloatAll,
    Float,
    Dock,
    ToggleFloat,
    ShowAll,
    Desktop,
    ToggleDesktop,
    ZoomIn,
    ZoomOut,
    Focus,
    Frame,
    Back,
    Forward,
    SaveView,
    LoadView,
    Left,
    Right,
    Up,
    Down,
    NextTab,
    PreviousTab,
    CloseTab,
    ExcludeApp,
    IncludeApp,
    SplitHorizontal,
    SplitVertical,
    Undo,
    Redo,
    NewWindow,
    Status,
}

impl Action {
    pub fn name(self) -> String {
        self.to_possible_value()
            .expect("workspace action")
            .get_name()
            .to_owned()
    }
}

pub fn state_path() -> Result<PathBuf> {
    Ok(crate::config::project_dirs()?
        .data_local_dir()
        .join("workspace.json"))
}

fn validate_snapshot(payload: &str) -> Result<Value> {
    anyhow::ensure!(
        payload.len() <= MAX_STATE_BYTES,
        "workspace snapshot is too large"
    );
    let value: Value = serde_json::from_str(payload).context("parse workspace snapshot")?;
    anyhow::ensure!(
        value["version"] == 1,
        "unsupported workspace snapshot version"
    );
    let model = &value["model"];
    anyhow::ensure!(
        model["version"] == 1
            && model["scopes"].is_object()
            && model["windows"].is_object()
            && model["nextId"].as_u64().is_some(),
        "invalid workspace model"
    );
    anyhow::ensure!(
        value["originals"].is_object(),
        "missing window recovery information"
    );
    // Semantic tree validation is shared with the runtime in Workspace.js.
    Ok(value)
}

fn atomic_write(path: &Path, payload: &str, backup: bool) -> Result<()> {
    let parent = path.parent().context("workspace path has no parent")?;
    fs::create_dir_all(parent)?;
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let temporary = parent.join(format!(".workspace-{}-{nonce}.tmp", std::process::id()));
    let result = (|| -> Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temporary)?;
        file.write_all(payload.as_bytes())?;
        file.sync_all()?;
        if backup && path.exists() {
            let previous = fs::read_to_string(path)?;
            if validate_snapshot(&previous).is_ok() {
                atomic_write(&path.with_extension("json.bak"), &previous, false)?;
            }
        }
        fs::rename(&temporary, path)?;
        fs::File::open(parent)?.sync_all()?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn load_snapshot(path: &Path) -> Result<String> {
    if !path.exists() {
        return Ok("{}".into());
    }
    let primary = fs::read_to_string(path).context("read workspace state");
    if let Ok(ref text) = primary {
        if validate_snapshot(text).is_ok() {
            return Ok(text.clone());
        }
    }
    let backup = fs::read_to_string(path.with_extension("json.bak"))
        .context("workspace state is invalid and no usable backup exists")?;
    validate_snapshot(&backup).context("workspace backup is invalid")?;
    Ok(backup)
}

#[derive(Clone, Default)]
struct SessionService {
    status: Arc<Mutex<String>>,
    quit: Arc<tokio::sync::Notify>,
}

fn dbus_error(error: impl std::fmt::Display) -> zbus::fdo::Error {
    zbus::fdo::Error::Failed(error.to_string())
}

#[zbus::interface(name = "com.benwbooth.FanzyZones")]
impl SessionService {
    fn quit(&self) {
        self.quit.notify_one();
    }
    fn load_workspace(&self) -> zbus::fdo::Result<String> {
        load_snapshot(&state_path().map_err(dbus_error)?).map_err(dbus_error)
    }

    fn save_workspace(&self, payload: String) -> zbus::fdo::Result<()> {
        validate_snapshot(&payload).map_err(dbus_error)?;
        atomic_write(&state_path().map_err(dbus_error)?, &payload, true).map_err(dbus_error)
    }

    fn report_status(&self, payload: String) -> zbus::fdo::Result<()> {
        if payload.len() >= 1024 * 1024 {
            return Err(dbus_error("runtime status is too large"));
        }
        serde_json::from_str::<Value>(&payload).map_err(dbus_error)?;
        *self.status.lock().map_err(dbus_error)? = payload;
        Ok(())
    }

    fn status(&self) -> zbus::fdo::Result<String> {
        let value = self.status.lock().map_err(dbus_error)?;
        Ok(if value.is_empty() {
            "{}".into()
        } else {
            value.clone()
        })
    }

    async fn invoke_action(&self, payload: String) -> zbus::fdo::Result<String> {
        crate::invoke_action_payload(&payload)
            .await
            .map_err(dbus_error)
    }

    async fn open_file_manager(&self) -> zbus::fdo::Result<()> {
        Command::new("dolphin")
            .arg("--new-window")
            .spawn()
            .map_err(dbus_error)?;
        Ok(())
    }
}

pub async fn serve() -> Result<()> {
    let service = SessionService::default();
    let quit = service.quit.clone();
    let _connection = zbus::connection::Builder::session()?
        .name(crate::FANZY_DBUS_SERVICE)?
        .serve_at(crate::FANZY_DBUS_PATH, service)?
        .build()
        .await?;
    tokio::select! {
        result = tokio::signal::ctrl_c() => { result?; }
        _ = quit.notified() => {}
    }
    Ok(())
}

pub async fn install_service(wrapper: &Path) -> Result<()> {
    let dir = crate::xdg_data_home()?.join("dbus-1/services");
    fs::create_dir_all(&dir)?;
    let executable = wrapper
        .to_string_lossy()
        .replace('\\', "\\\\")
        .replace('"', "\\\"");
    anyhow::ensure!(
        !executable.contains(['\n', '\r']),
        "invalid executable path"
    );
    fs::write(
        dir.join(format!("{}.service", crate::FANZY_DBUS_SERVICE)),
        format!(
            "[D-BUS Service]\nName={}\nExec=\"{}\" serve-workspace\n",
            crate::FANZY_DBUS_SERVICE,
            executable
        ),
    )?;
    let applications = crate::xdg_data_home()?.join("applications");
    fs::create_dir_all(&applications)?;
    fs::write(applications.join("fanzyzones-workspace-url.desktop"), format!(
        "[Desktop Entry]\nType=Application\nName=FanzyZones Workspace\nExec=\"{}\" handle-url %u\nIcon=fanzyzones-kde\nNoDisplay=true\nMimeType=x-scheme-handler/fanzyzones;\n", executable
    ))?;
    checked(
        "xdg-mime",
        &[
            "default",
            "fanzyzones-workspace-url.desktop",
            "x-scheme-handler/fanzyzones",
        ],
    )
    .await?;
    // Let D-Bus activate the newly installed executable next time. Older
    // versions without this method may keep running until logout.
    let _ = checked(
        "busctl",
        &[
            "--user",
            "--auto-start=no",
            "call",
            crate::FANZY_DBUS_SERVICE,
            crate::FANZY_DBUS_PATH,
            crate::FANZY_DBUS_INTERFACE,
            "Quit",
        ],
    )
    .await;
    Ok(())
}

pub fn effect_dir() -> Result<PathBuf> {
    if let Some(dir) = std::env::var_os("FANZYZONES_KDE_EFFECT_DIR") {
        return Ok(dir.into());
    }
    let candidates = [
        crate::resources::resource_root().join("kwin-effect"),
        std::env::current_exe()?
            .parent()
            .context("missing binary directory")?
            .join("../share/fanzyzones-kde/kwin-effect"),
        std::env::current_dir()?.join("kwin-effect"),
    ];
    candidates
        .into_iter()
        .find(|p| p.join("metadata.json").exists())
        .context("nested workspace effect was not found")
}

async fn checked(program: &str, args: &[&str]) -> Result<String> {
    let result = Command::new(program)
        .args(args)
        .output()
        .await
        .with_context(|| format!("run {program}"))?;
    anyhow::ensure!(
        result.status.success(),
        "{program}: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    Ok(String::from_utf8_lossy(&result.stdout).trim().to_owned())
}

pub async fn install_effect() -> Result<()> {
    let path = effect_dir()?.to_string_lossy().into_owned();
    let installed = checked("kpackagetool6", &["--type", "KWin/Effect", "--list"])
        .await?
        .lines()
        .any(|l| l.trim() == EFFECT_ID);
    // KPackage rejects same-version upgrades, including reinstalling a local
    // development build. Follow the script installer and replace the package.
    if installed {
        checked(
            "kpackagetool6",
            &["--type", "KWin/Effect", "--remove", EFFECT_ID],
        )
        .await?;
    }
    checked(
        "kpackagetool6",
        &["--type", "KWin/Effect", "--install", &path],
    )
    .await?;
    let native_target = crate::xdg_data_home()?
        .join("kwin/effects")
        .join(EFFECT_ID)
        .join("contents/ui/native");
    let native_source = std::env::var_os("FANZYZONES_KDE_NATIVE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::resources::resource_root().join("native"));
    if !native_target.join("libfanzyzonesinput.so").exists()
        && native_source.join("libfanzyzonesinput.so").is_file()
        && native_source.join("qmldir").is_file()
    {
        crate::copy_dir_all(&native_source, &native_target)?;
    }
    Ok(())
}

pub async fn write_config(settings: &crate::config::Settings) -> Result<()> {
    checked(
        "kwriteconfig6",
        &[
            "--file",
            "kwinrc",
            "--group",
            &format!("Effect-{EFFECT_ID}"),
            "--key",
            "settingsJson",
            &settings.compact_json()?,
        ],
    )
    .await?;
    checked(
        "kwriteconfig6",
        &[
            "--file",
            "kwinrc",
            "--group",
            "Plugins",
            "--key",
            &format!("{EFFECT_ID}Enabled"),
            if settings.nested_enabled {
                "true"
            } else {
                "false"
            },
        ],
    )
    .await?;
    Ok(())
}

async fn effect_method(method: &str) -> Result<String> {
    checked(
        "busctl",
        &[
            "--user",
            "call",
            "org.kde.KWin",
            "/Effects",
            "org.kde.kwin.Effects",
            method,
            "s",
            EFFECT_ID,
        ],
    )
    .await
}

pub async fn refresh_effect() -> Result<()> {
    if effect_method("isEffectLoaded").await? == "b true" {
        effect_method("reconfigureEffect").await?;
    }
    Ok(())
}

pub async fn status() -> Result<Value> {
    let connection = zbus::Connection::session().await?;
    let proxy = zbus::Proxy::new(
        &connection,
        crate::FANZY_DBUS_SERVICE,
        crate::FANZY_DBUS_PATH,
        crate::FANZY_DBUS_INTERFACE,
    )
    .await?;
    let mut value: String = proxy.call("Status", &()).await?;
    if value == "{}" {
        // A reactivated helper has no live report yet. Ask the loaded effect
        // to publish one; keep an unloaded/disabled workspace inexpensive.
        refresh_effect().await?;
        for _ in 0..10 {
            tokio::time::sleep(Duration::from_millis(20)).await;
            value = proxy.call("Status", &()).await?;
            if value != "{}" {
                break;
            }
        }
    }
    Ok(serde_json::from_str(&value)?)
}

pub async fn dispatch(mut payload: Value) -> Result<Value> {
    let action = payload["action"]
        .as_str()
        .context("workspace action is missing")?
        .to_owned();
    Action::from_str(&action, false).map_err(anyhow::Error::msg)?;
    if action == "status" {
        return status().await;
    }
    if action == "save-view" || action == "load-view" {
        anyhow::ensure!(
            matches!(payload["slot"].as_u64(), Some(1..=9)),
            "view must be 1 through 9"
        );
    }
    let mut settings = crate::load_and_save_settings()?;
    let controller = crate::KwinController::from_environment()?;
    if action == "enable" {
        // Installation is a separate operation. Enabling an already installed
        // effect must not overwrite a distro's matching native input module.
        let local = crate::xdg_data_home()?
            .join("kwin/effects")
            .join(EFFECT_ID)
            .join("metadata.json");
        if !local.exists() {
            install_effect().await?;
        }
        settings.nested_enabled = true;
        crate::config::save(&settings)?;
        controller.write_settings(&settings).await?;
        write_config(&settings).await?;
        controller.reload_runtime_settings().await?;
    } else if action != "disable" {
        anyhow::ensure!(
            settings.nested_enabled,
            "enable nested workspaces first: fanzyzones-kde workspace enable"
        );
    }
    if effect_method("isEffectLoaded").await? != "b true" {
        anyhow::ensure!(
            effect_method("loadEffect").await? == "b true",
            "KWin could not load the workspace effect; inspect the KWin journal"
        );
    }
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_nanos()
        .to_string();
    payload["nonce"] = Value::String(nonce.clone());
    checked(
        "kwriteconfig6",
        &[
            "--file",
            "kwinrc",
            "--group",
            &format!("Effect-{EFFECT_ID}"),
            "--key",
            "pendingActionJson",
            &serde_json::to_string(&payload)?,
        ],
    )
    .await?;
    effect_method("reconfigureEffect").await?;
    let mut response = None;
    for _ in 0..80 {
        if let Ok(value) = status().await {
            if value["lastAction"].as_str() == Some(&nonce) {
                response = Some(value);
                break;
            }
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let response = response
        .context("KWin did not acknowledge the workspace action; no success was assumed")?;
    if let Some(error) = response["error"].as_str().filter(|e| !e.is_empty()) {
        bail!("workspace action failed: {error}");
    }
    if action == "disable" {
        settings.nested_enabled = false;
        crate::config::save(&settings)?;
        controller.write_settings(&settings).await?;
        write_config(&settings).await?;
        effect_method("unloadEffect").await?;
        controller.reload_runtime_settings().await?;
    }
    Ok(response)
}

fn decode_component(value: &str) -> Result<String> {
    let bytes = value.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' => {
                anyhow::ensure!(i + 2 < bytes.len(), "incomplete URL escape");
                let h = (bytes[i + 1] as char)
                    .to_digit(16)
                    .context("invalid URL escape")?;
                let l = (bytes[i + 2] as char)
                    .to_digit(16)
                    .context("invalid URL escape")?;
                out.push((h * 16 + l) as u8);
                i += 3;
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8(out).context("URL is not UTF-8")
}

pub fn parse_url(url: &str) -> Result<Value> {
    let rest = url
        .strip_prefix("fanzyzones://")
        .context("expected a fanzyzones:// URL")?;
    anyhow::ensure!(!rest.contains('#'), "URL fragments are not supported");
    let (route, query) = rest.split_once('?').unwrap_or((rest, ""));
    let action =
        Action::from_str(route.trim_end_matches('/'), false).map_err(anyhow::Error::msg)?;
    let mut payload = json!({ "action": action.name() });
    for pair in query.split('&').filter(|p| !p.is_empty()) {
        let (key, value) = pair
            .split_once('=')
            .context("URL parameter needs a value")?;
        let value = decode_component(value)?;
        match key {
            "slot" => {
                let slot: u8 = value.parse()?;
                anyhow::ensure!((1..=9).contains(&slot), "view must be 1 through 9");
                payload[key] = json!(slot);
            }
            "display" | "window" | "app" | "target" => payload[key] = json!(value),
            "edge" => {
                anyhow::ensure!(
                    ["left", "right", "top", "bottom", "center"].contains(&value.as_str()),
                    "invalid docking edge"
                );
                payload[key] = json!(value);
            }
            "group" => {
                anyhow::ensure!(
                    value == "true" || value == "false",
                    "group must be true or false"
                );
                payload[key] = json!(value == "true");
            }
            "rect" => {
                let rect: Vec<f64> = value
                    .split(',')
                    .map(str::parse)
                    .collect::<std::result::Result<_, _>>()?;
                anyhow::ensure!(
                    rect.len() == 4
                        && rect.iter().all(|x| x.is_finite())
                        && rect[2] > 0.
                        && rect[3] > 0.,
                    "invalid frame rectangle"
                );
                payload[key] = json!({"x":rect[0], "y":rect[1], "width":rect[2], "height":rect[3]});
            }
            _ => bail!("unknown workspace URL parameter: {key}"),
        }
    }
    Ok(payload)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn urls_are_typed_and_never_shell_commands() {
        assert_eq!(
            parse_url("fanzyzones://save-view?slot=9&display=DP%2D1").unwrap(),
            json!({"action":"save-view","slot":9,"display":"DP-1"})
        );
        assert!(parse_url("fanzyzones://save-view?slot=0").is_err());
        assert!(parse_url("fanzyzones://exec?command=anything").is_err());
        assert!(parse_url("fanzyzones://frame?rect=0,0,-1,1").is_err());
        assert!(parse_url("fanzyzones://focus?window=%zz").is_err());
        assert!(parse_url("https://example.com").is_err());
    }
    fn snapshot(n: u64) -> String {
        json!({"version":1,"model":{"version":1,"nextId":n,"scopes":{},"windows":{}},"originals":{}}).to_string()
    }
    #[test]
    fn atomic_storage_recovers_previous_valid_snapshot() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("workspace.json");
        assert_eq!(load_snapshot(&path).unwrap(), "{}");
        atomic_write(&path, &snapshot(1), true).unwrap();
        atomic_write(&path, &snapshot(2), true).unwrap();
        assert_eq!(load_snapshot(&path).unwrap(), snapshot(2));
        fs::write(&path, "partial write").unwrap();
        assert_eq!(load_snapshot(&path).unwrap(), snapshot(1));
    }
    #[test]
    fn invalid_state_is_not_silently_discarded() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("workspace.json");
        fs::write(&path, "corrupt").unwrap();
        assert!(load_snapshot(&path).is_err());
        assert!(validate_snapshot("{}").is_err());
    }
}
