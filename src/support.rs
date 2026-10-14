//! User-invoked release checks and a deliberately small, reviewable report.
use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::time::Duration;
use tokio::process::Command;

const RELEASES: &str = "https://github.com/benwbooth/fanzyzones-kde/releases";

fn version(value: &str) -> Option<[u64; 3]> {
    let components: Vec<_> = value.trim_start_matches('v').split('.').collect();
    if components.len() != 3 {
        return None;
    }
    Some([
        components[0].parse().ok()?,
        components[1].parse().ok()?,
        components[2].parse().ok()?,
    ])
}

fn release_result(payload: &str) -> Result<Value> {
    let release: Value = serde_json::from_str(payload).context("invalid release response")?;
    let tag = release["tag_name"]
        .as_str()
        .context("release has no version")?;
    let latest = version(tag).context("release version is not a stable version")?;
    anyhow::ensure!(
        release["draft"] == false && release["prerelease"] == false,
        "release is not stable"
    );
    Ok(json!({"current": env!("CARGO_PKG_VERSION"), "latest": tag,
        "updateAvailable": latest > version(env!("CARGO_PKG_VERSION")).expect("package version"),
        "url": format!("{RELEASES}/tag/{tag}")}))
}

pub async fn check_updates() -> Result<Value> {
    let output = Command::new("curl")
        .args([
            "--fail",
            "--silent",
            "--show-error",
            "--max-time",
            "15",
            "--max-filesize",
            "1048576",
            "--proto",
            "=https",
            "--header",
            "Accept: application/vnd.github+json",
            "--user-agent",
            "fanzyzones-kde",
            "https://api.github.com/repos/benwbooth/fanzyzones-kde/releases/latest",
        ])
        .output()
        .await
        .context("check releases (curl is required)")?;
    anyhow::ensure!(
        output.status.success(),
        "Release check failed: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    );
    release_result(&String::from_utf8(output.stdout)?)
}

fn report(settings: &crate::config::Settings, runtime: &Value, kwin: &str) -> String {
    // Do not copy the model, window titles, application IDs, paths, or logs.
    let summary = json!({
        "version": env!("CARGO_PKG_VERSION"), "kwin": kwin,
        "platform": std::env::consts::OS, "architecture": std::env::consts::ARCH,
        "session": std::env::var("XDG_SESSION_TYPE").unwrap_or_default(),
        "nestedEnabled": settings.nested_enabled, "animationMs": settings.nested_animation_ms,
        "gap": settings.nested_gap, "excludedApplicationCount": settings.nested_excluded_apps.len(),
        "ready": runtime["ready"].as_bool(), "nativeInput": runtime["nativeInput"].as_bool(),
        "managedWindows": runtime["windows"].as_u64(),
        "displayCount": runtime["displays"].as_array().map(Vec::len),
        "animating": runtime["animating"].as_bool(),
    });
    format!("What happened?\n\n[Describe the problem]\n\nSteps to reproduce\n\n1. \n\nExpected behavior\n\n[Describe what you expected]\n\nDiagnostics\n\n```json\n{}\n```\n", serde_json::to_string_pretty(&summary).unwrap())
}

pub async fn diagnostics() -> Result<Value> {
    let settings = crate::config::load_or_default()?;
    let runtime = tokio::time::timeout(Duration::from_secs(3), crate::workspace::status())
        .await
        .ok()
        .and_then(Result::ok)
        .unwrap_or(Value::Null);
    let kwin = tokio::time::timeout(Duration::from_secs(3), async {
        let connection = zbus::Connection::session().await?;
        let proxy = zbus::Proxy::new(&connection, "org.kde.KWin", "/KWin", "org.kde.KWin").await?;
        let information: String = proxy.call("supportInformation", &()).await?;
        Ok::<_, zbus::Error>(
            information
                .lines()
                .find(|line| line.starts_with("KWin version:"))
                .unwrap_or("KWin version unavailable")
                .to_owned(),
        )
    })
    .await
    .ok()
    .and_then(Result::ok)
    .unwrap_or_else(|| "unavailable".into());
    Ok(
        json!({"draft": report(&settings, &runtime, &kwin), "url": "https://github.com/benwbooth/fanzyzones-kde/issues/new"}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compares_release_numbers_and_rejects_untrusted_tags() {
        assert!(version("v0.10.0") > version("0.9.9"));
        let result =
            release_result(r#"{"tag_name":"v99.0.0","draft":false,"prerelease":false}"#).unwrap();
        assert_eq!(result["updateAvailable"], true);
        for tag in [
            "v0.3.1-beta",
            "https://example.com",
            "v1.2.3?body=bad",
            "1.2",
        ] {
            assert!(version(tag).is_none());
        }
    }
    #[test]
    fn report_omits_private_workspace_data() {
        let runtime = json!({"ready":true,"windows":1,"nativeInput":true,"displays":[{"name":"private display"}],
            "model":{"windows":{"secret":{"title":"private document","appId":"private app"}}},
            "error":"private error with path"});
        let result = report(&crate::config::Settings::default(), &runtime, "KWin 6.6");
        assert!(result.contains("KWin 6.6"));
        assert!(!result.contains("private"));
        assert!(!result.contains("secret"));
    }
}
