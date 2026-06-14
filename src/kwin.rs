use crate::config::Settings;
use anyhow::{anyhow, bail, Context, Result};
use std::env;
use std::path::{Path, PathBuf};
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
        let script_dir = if let Ok(path) = env::var("FANZYZONES_KDE_KWIN_SCRIPT_DIR") {
            PathBuf::from(path)
        } else {
            env::current_exe()
                .ok()
                .and_then(|exe| exe.parent().map(Path::to_path_buf))
                .map(|bin| bin.join("../share/fanzyzones-kde/kwin-script"))
                .unwrap_or_else(|| PathBuf::from("kwin-script"))
        };

        let fallback = env::current_dir()?.join("kwin-script");
        let script_dir = if script_dir.exists() {
            script_dir
        } else if fallback.exists() {
            fallback
        } else {
            script_dir
        };

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
        let action = if installed { "--upgrade" } else { "--install" };
        let script_dir = self.script_dir.to_string_lossy().into_owned();
        run_checked(
            "kpackagetool6",
            &["--type", "KWin/Script", action, &script_dir],
        )
        .await
        .with_context(|| format!("{} KWin script from {}", action, self.script_dir.display()))?;
        Ok(())
    }

    pub async fn is_installed(&self) -> Result<bool> {
        let result = run("kpackagetool6", &["--type", "KWin/Script", "--list"]).await?;
        Ok(result.stdout.lines().any(|line| line.contains(SCRIPT_ID)))
    }

    pub async fn write_settings(&self, settings: &Settings) -> Result<()> {
        let json = settings.compact_json()?;
        let group = format!("Script-{}", SCRIPT_ID);
        run_checked(
            "kwriteconfig6",
            &[
                "--file",
                "kwinrc",
                "--group",
                &group,
                "--key",
                "settingsJson",
                &json,
            ],
        )
        .await
        .context("write FanzyZones settings to kwinrc")?;
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

    pub async fn invoke_shortcut(&self, shortcut: &str) -> Result<()> {
        let candidates: &[(&str, &[&str])] = &[
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

    pub async fn sync(&self, settings: &Settings, reload: bool) -> Result<()> {
        self.install_or_upgrade().await?;
        self.write_settings(settings).await?;
        self.enable_script().await?;
        if reload {
            self.reload_kwin().await?;
        }
        Ok(())
    }
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
