use crate::config::Settings;
use anyhow::{anyhow, bail, Context, Result};
use serde_json::json;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::process::Command;

pub const SCRIPT_ID: &str = "fanzyzones_kde";

#[derive(Debug, Clone)]
pub struct KwinController {
    script_dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandResult {
    pub program: String,
    pub status: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

impl CommandResult {
    fn ensure_success(self) -> Result<Self> {
        if self.status == Some(0) {
            Ok(self)
        } else {
            Err(anyhow!(
                "{} exited with {:?}\nstdout:\n{}\nstderr:\n{}",
                self.program,
                self.status,
                self.stdout,
                self.stderr
            ))
        }
    }
}

impl KwinController {
    pub fn new(script_dir: PathBuf) -> Self {
        Self { script_dir }
    }

    pub fn from_environment() -> Result<Self> {
        if let Ok(path) = env::var("FANZYZONES_KDE_KWIN_SCRIPT_DIR") {
            return Ok(Self::new(PathBuf::from(path)));
        }

        // A self-contained binary unpacks here; then the Nix-install layout, then
        // a repo checkout.
        let candidates = [
            crate::resources::resource_root().join("kwin-script"),
            env::current_exe()
                .ok()
                .and_then(|exe| exe.parent().map(Path::to_path_buf))
                .map(|bin| bin.join("../share/fanzyzones-kde/kwin-script"))
                .unwrap_or_default(),
            env::current_dir().ok().map(|d| d.join("kwin-script")).unwrap_or_default(),
        ];
        let script_dir = candidates
            .iter()
            .find(|path| path.exists())
            .cloned()
            .unwrap_or_else(|| candidates[0].clone());

        Ok(Self::new(script_dir))
    }

    pub fn script_dir(&self) -> &Path {
        &self.script_dir
    }

    pub async fn install_or_upgrade(&self) -> Result<()> {
        if !self.script_dir.join("metadata.json").exists() {
            bail!(
                "KWin script metadata not found in {}",
                self.script_dir.display()
            );
        }

        let installed = self.is_installed().await.unwrap_or(false);
        if installed {
            run_checked(
                "kpackagetool6",
                &["--type", "KWin/Script", "--remove", SCRIPT_ID],
            )
            .await
            .context("remove previous FanzyZones KWin script package")?;
        }

        let script_dir = self.script_dir.to_string_lossy().into_owned();
        run_checked(
            "kpackagetool6",
            &["--type", "KWin/Script", "--install", &script_dir],
        )
        .await
        .with_context(|| format!("install KWin script from {}", self.script_dir.display()))?;
        Ok(())
    }

    pub async fn is_installed(&self) -> Result<bool> {
        let result = run("kpackagetool6", &["--type", "KWin/Script", "--list"]).await?;
        Ok(result.stdout.lines().any(|line| line.contains(SCRIPT_ID)))
    }

    /// Turn off KWin's built-in drag-to-edge quick tiling/maximize so it does
    /// not fight FanzyZones' own Shift+drag snapping.
    pub async fn disable_builtin_tiling(&self) -> Result<()> {
        for key in ["ElectricBorderTiling", "ElectricBorderMaximize"] {
            run_checked(
                "kwriteconfig6",
                &["--file", "kwinrc", "--group", "Windows", "--key", key, "false"],
            )
            .await
            .with_context(|| format!("disable KWin {key}"))?;
        }
        Ok(())
    }

    pub async fn write_settings(&self, settings: &Settings) -> Result<()> {
        let json = settings.compact_json()?;
        self.write_script_config("settingsJson", &json)
            .await
            .context("write FanzyZones settings to kwinrc")?;
        Ok(())
    }

    pub async fn write_script_config(&self, key: &str, value: &str) -> Result<()> {
        let group = format!("Script-{}", SCRIPT_ID);
        run_checked(
            "kwriteconfig6",
            &["--file", "kwinrc", "--group", &group, "--key", key, value],
        )
        .await
        .with_context(|| format!("write FanzyZones KWin config key {}", key))?;
        Ok(())
    }

    pub async fn enable_script(&self) -> Result<()> {
        let key = format!("{}Enabled", SCRIPT_ID);
        run_checked(
            "kwriteconfig6",
            &[
                "--file", "kwinrc", "--group", "Plugins", "--key", &key, "true",
            ],
        )
        .await
        .context("enable FanzyZones KWin script")?;
        Ok(())
    }

    pub async fn disable_script(&self) -> Result<()> {
        let key = format!("{}Enabled", SCRIPT_ID);
        run_checked(
            "kwriteconfig6",
            &[
                "--file", "kwinrc", "--group", "Plugins", "--key", &key, "false",
            ],
        )
        .await
        .context("disable FanzyZones KWin script")?;
        Ok(())
    }

    pub async fn reload_kwin(&self) -> Result<()> {
        let candidates: &[(&str, &[&str])] = &[
            (
                "qdbus6",
                &["org.kde.KWin", "/KWin", "org.kde.KWin.reconfigure"],
            ),
            ("qdbus6", &["org.kde.KWin", "/KWin", "reconfigure"]),
            (
                "qdbus",
                &["org.kde.KWin", "/KWin", "org.kde.KWin.reconfigure"],
            ),
            ("qdbus", &["org.kde.KWin", "/KWin", "reconfigure"]),
        ];

        let mut failures = Vec::new();
        for (program, args) in candidates {
            match run(program, args).await {
                Ok(result) if result.status == Some(0) => return Ok(()),
                Ok(result) => failures.push(format!(
                    "{} {:?} exited {:?}: {}{}",
                    program, args, result.status, result.stdout, result.stderr
                )),
                Err(err) => failures.push(format!("{} {:?}: {}", program, args, err)),
            }
        }

        bail!(
            "could not ask KWin to reconfigure; log out/in or restart KWin\n{}",
            failures.join("\n")
        );
    }

    pub async fn restart_script(&self) -> Result<()> {
        let installed_script = installed_script_main();
        let runtime_script = runtime_script_main(&installed_script)?;
        let installed_script = installed_script.to_string_lossy().into_owned();
        let runtime_script = runtime_script.to_string_lossy().into_owned();
        let old_installed_script = old_installed_script_main();
        let old_installed_script = old_installed_script.to_string_lossy().into_owned();

        let mut unload_keys = vec![
            SCRIPT_ID.to_string(),
            installed_script.clone(),
            old_installed_script.clone(),
        ];
        unload_keys.extend(
            runtime_script_paths()
                .into_iter()
                .map(|path| path.to_string_lossy().into_owned()),
        );

        for key in unload_keys {
            let _ = run(
                "busctl",
                &[
                    "--user",
                    "call",
                    "org.kde.KWin",
                    "/Scripting",
                    "org.kde.kwin.Scripting",
                    "unloadScript",
                    "s",
                    &key,
                ],
            )
            .await;
        }

        // With the script unloaded, drop global shortcuts whose actions are no
        // longer registered (stale entries from renamed/removed shortcuts or
        // other uninstalled tiling scripts). This lets the reload below register
        // our shortcuts fresh so their default keys actually bind.
        let _ = run(
            "busctl",
            &[
                "--user",
                "call",
                "org.kde.kglobalaccel",
                "/component/kwin",
                "org.kde.kglobalaccel.Component",
                "cleanUp",
            ],
        )
        .await;

        run_checked(
            "busctl",
            &[
                "--user",
                "call",
                "org.kde.KWin",
                "/Scripting",
                "org.kde.kwin.Scripting",
                "loadDeclarativeScript",
                "ss",
                &runtime_script,
                SCRIPT_ID,
            ],
        )
        .await
        .context("load FanzyZones KWin script")?;

        run_checked(
            "busctl",
            &[
                "--user",
                "call",
                "org.kde.KWin",
                "/Scripting",
                "org.kde.kwin.Scripting",
                "start",
            ],
        )
        .await
        .context("start FanzyZones KWin script")?;

        Ok(())
    }

    pub async fn invoke_shortcut(&self, shortcut: &str) -> Result<()> {
        let candidates: &[(&str, &[&str])] = &[
            (
                "busctl",
                &[
                    "--user",
                    "call",
                    "org.kde.kglobalaccel",
                    "/component/kwin",
                    "org.kde.kglobalaccel.Component",
                    "invokeShortcut",
                    "s",
                    shortcut,
                ],
            ),
            (
                "qdbus6",
                &[
                    "org.kde.kglobalaccel",
                    "/component/kwin",
                    "org.kde.kglobalaccel.Component.invokeShortcut",
                    shortcut,
                ],
            ),
            (
                "qdbus",
                &[
                    "org.kde.kglobalaccel",
                    "/component/kwin",
                    "org.kde.kglobalaccel.Component.invokeShortcut",
                    shortcut,
                ],
            ),
        ];

        let mut failures = Vec::new();
        for (program, args) in candidates {
            match run(program, args).await {
                Ok(result) if result.status == Some(0) => return Ok(()),
                Ok(result) => failures.push(format!(
                    "{} exited {:?}: {}{}",
                    program, result.status, result.stdout, result.stderr
                )),
                Err(err) => failures.push(format!("{}: {}", program, err)),
            }
        }

        bail!(
            "could not invoke KWin shortcut '{}'\n{}",
            shortcut,
            failures.join("\n")
        );
    }

    pub async fn set_runtime_layout(&self, _layout_index: usize) -> Result<()> {
        self.restart_script().await
    }

    pub async fn snap_active_window_to_layout_zone(
        &self,
        settings: &Settings,
        layout_index: usize,
        zone_index: usize,
    ) -> Result<()> {
        let layout = settings
            .layouts
            .get(layout_index)
            .with_context(|| format!("layout {} does not exist", layout_index + 1))?;
        let zone = layout
            .zones
            .get(zone_index)
            .with_context(|| format!("zone {} does not exist", zone_index + 1))?;

        let action = json!({
            "layoutIndex": layout_index,
            "zoneIndex": zone_index,
            "layoutPadding": layout.padding,
            "gap": settings.gap,
            "outerPadding": settings.outer_padding,
            "skippedWindowClasses": settings.skipped_window_classes,
            "zone": zone,
        });
        let script = format!(
            r#"import QtQuick
import org.kde.kwin

Item {{
    property var action: ({action})

    function windowClass(client) {{
        if (!client || !client.resourceClass)
            return "";
        return client.resourceClass.toString().toLowerCase();
    }}

    function windowCaption(client) {{
        if (!client || !client.caption)
            return "";
        return client.caption.toString().toLowerCase();
    }}

    function isFanzyZonesWindow(client) {{
        const caption = windowCaption(client);
        return caption === "fanzyzones" || caption.indexOf("fanzyzones ") === 0;
    }}

    function isSkippedWindow(client) {{
        if (!client)
            return true;
        if (isFanzyZonesWindow(client))
            return true;
        if (!client.normalWindow || client.skipTaskbar || client.popupWindow || client.desktopWindow || client.dock)
            return true;
        const klass = windowClass(client);
        return !klass || action.skippedWindowClasses.indexOf(klass) !== -1;
    }}

    function windowsInStackingOrder() {{
        if (Workspace.stackingOrder)
            return Workspace.stackingOrder;
        if (Workspace.windowList)
            return Workspace.windowList();
        return [];
    }}

    function clientOnCurrentDesktop(client) {{
        if (!client)
            return false;
        if (client.onAllDesktops)
            return true;
        if (!client.desktops || client.desktops.length === 0)
            return true;
        return client.desktops.indexOf(Workspace.currentDesktop) !== -1;
    }}

    function isCandidateWindow(client) {{
        if (isSkippedWindow(client))
            return false;
        if (client.minimized || client.hidden || client.hiddenByShowDesktop)
            return false;
        return clientOnCurrentDesktop(client);
    }}

    function targetWindow() {{
        const active = Workspace.activeWindow;
        if (isCandidateWindow(active))
            return active;

        const all = windowsInStackingOrder();
        for (let i = all.length - 1; i >= 0; i--) {{
            const client = all[i];
            if (client === active)
                continue;
            if (isCandidateWindow(client))
                return client;
        }}
        return null;
    }}

    Component.onCompleted: {{
        const client = targetWindow();
        if (isSkippedWindow(client))
            return;

        const screen = client.screen ? client.screen : Workspace.activeScreen;
        let area;
        try {{
            area = Workspace.clientArea(KWin.MaximizeArea, screen, Workspace.currentDesktop);
        }} catch (error) {{
            area = Workspace.clientArea(KWin.FullScreenArea, screen, Workspace.currentDesktop);
        }}

        const zone = action.zone;
        const padding = Math.max(0, action.outerPadding || 0) + Math.max(0, action.layoutPadding || 0);
        const gap = Math.max(0, action.gap || 0);
        const usableX = area.x + padding;
        const usableY = area.y + padding;
        const usableWidth = Math.max(1, area.width - padding * 2);
        const usableHeight = Math.max(1, area.height - padding * 2);
        const rect = Qt.rect(
            Math.round(usableX + usableWidth * zone.x + gap / 2),
            Math.round(usableY + usableHeight * zone.y + gap / 2),
            Math.max(1, Math.round(usableWidth * zone.width - gap)),
            Math.max(1, Math.round(usableHeight * zone.height - gap))
        );

        if (client.setMaximize)
            client.setMaximize(false, false);
        client.frameGeometry = rect;
        client.fanzyZone = action.zoneIndex;
        client.fanzyLayout = action.layoutIndex;
        client.fanzyDesktop = Workspace.currentDesktop;
    }}
}}
"#
        );

        self.run_one_shot_script("snap", &script).await
    }

    pub async fn reload_runtime_settings(&self) -> Result<()> {
        self.restart_script().await
    }

    async fn run_one_shot_script(&self, action_name: &str, source: &str) -> Result<()> {
        let cache_buster = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .context("read system time")?
            .as_millis();
        let script_id = format!("{SCRIPT_ID}_{action_name}_{cache_buster}");
        let script_dir = runtime_script_root().join(format!("oneshot-{cache_buster}"));
        fs::create_dir_all(&script_dir)
            .with_context(|| format!("create {}", script_dir.display()))?;
        let script_path = script_dir.join("FanzyZonesAction.qml");
        fs::write(&script_path, source)
            .with_context(|| format!("write {}", script_path.display()))?;
        let script_path = script_path.to_string_lossy().into_owned();

        let load = run_checked(
            "busctl",
            &[
                "--user",
                "call",
                "org.kde.KWin",
                "/Scripting",
                "org.kde.kwin.Scripting",
                "loadDeclarativeScript",
                "ss",
                &script_path,
                &script_id,
            ],
        )
        .await
        .context("load FanzyZones one-shot KWin script")?;
        let script_index = parse_busctl_i32(&load.stdout)?;
        let script_object = format!("/Scripting/Script{script_index}");

        let run_result = run_checked(
            "busctl",
            &[
                "--user",
                "call",
                "org.kde.KWin",
                &script_object,
                "org.kde.kwin.Script",
                "run",
            ],
        )
        .await
        .context("run FanzyZones one-shot KWin script");

        let _ = run(
            "busctl",
            &[
                "--user",
                "call",
                "org.kde.KWin",
                "/Scripting",
                "org.kde.kwin.Scripting",
                "unloadScript",
                "s",
                &script_id,
            ],
        )
        .await;

        run_result.map(|_| ())
    }

    pub async fn sync(&self, settings: &Settings, reload: bool) -> Result<()> {
        self.install_or_upgrade().await?;
        self.write_settings(settings).await?;
        self.disable_builtin_tiling().await?;
        self.enable_script().await?;
        if reload {
            if self.restart_script().await.is_err() {
                self.reload_kwin().await?;
            }
            // Apply the kwinrc window-behaviour change (disabled tiling).
            let _ = self.reload_kwin().await;
        }
        Ok(())
    }
}

fn installed_script_main() -> PathBuf {
    home_dir()
        .join(".local/share/kwin/scripts")
        .join(SCRIPT_ID)
        .join("contents/ui/fanzyzones.qml")
}

fn old_installed_script_main() -> PathBuf {
    home_dir()
        .join(".local/share/kwin/scripts")
        .join(SCRIPT_ID)
        .join("contents/ui/main.qml")
}

fn runtime_script_main(installed_script: &Path) -> Result<PathBuf> {
    let cache_buster = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("read system time")?
        .as_millis();
    let script_dir = runtime_script_root().join(format!("reload-{cache_buster}"));
    fs::create_dir_all(&script_dir).with_context(|| format!("create {}", script_dir.display()))?;
    let runtime_script = script_dir.join("FanzyZones.qml");
    fs::copy(installed_script, &runtime_script).with_context(|| {
        format!(
            "copy {} to {}",
            installed_script.display(),
            runtime_script.display()
        )
    })?;
    Ok(runtime_script)
}

fn runtime_script_paths() -> Vec<PathBuf> {
    let root = runtime_script_root();
    let mut paths = Vec::new();
    collect_qml_files(&root, &mut paths, 0);
    paths
}

fn runtime_script_root() -> PathBuf {
    env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| home_dir().join(".cache"))
        .join("fanzyzones-kde")
        .join("kwin-script")
}

fn collect_qml_files(dir: &Path, paths: &mut Vec<PathBuf>, depth: usize) {
    if depth > 2 {
        return;
    }

    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_qml_files(&path, paths, depth + 1);
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("qml") {
            paths.push(path);
        }
    }
}

fn parse_busctl_i32(stdout: &str) -> Result<i32> {
    stdout
        .split_whitespace()
        .filter_map(|part| part.parse::<i32>().ok())
        .next()
        .with_context(|| format!("parse busctl int from '{}'", stdout.trim()))
}

fn home_dir() -> PathBuf {
    env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

async fn run_checked(program: &str, args: &[&str]) -> Result<CommandResult> {
    run(program, args).await?.ensure_success()
}

async fn run(program: &str, args: &[&str]) -> Result<CommandResult> {
    let output = Command::new(program)
        .args(args)
        .output()
        .await
        .with_context(|| format!("run {}", program))?;
    Ok(CommandResult {
        program: program.to_string(),
        status: output.status.code(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}
