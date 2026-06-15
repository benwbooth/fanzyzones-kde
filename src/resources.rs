//! The plasmoid, KWin script, and icons are embedded in the binary so a single
//! self-contained executable can install everything itself — no sibling files,
//! no tarball. On `install` (when not running from a checkout / Nix store where
//! the FANZYZONES_KDE_*_DIR env vars already point at real directories) the
//! binary unpacks them under ~/.local/share/fanzyzones-kde and registers them.

use anyhow::{Context, Result};
use include_dir::{include_dir, Dir};
use std::path::PathBuf;

static KWIN_SCRIPT: Dir = include_dir!("$CARGO_MANIFEST_DIR/kwin-script");
static PLASMOID: Dir = include_dir!("$CARGO_MANIFEST_DIR/plasma-applet");
static ICONS: Dir = include_dir!("$CARGO_MANIFEST_DIR/resources/icons");

/// Where the self-contained binary unpacks its embedded resources
/// (~/.local/share/fanzyzones-kde). Also where the CLI wrapper lives.
pub fn resource_root() -> PathBuf {
    crate::config::project_dirs()
        .map(|dirs| dirs.data_dir().to_path_buf())
        .unwrap_or_else(|_| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_default()
                .join(".local/share/fanzyzones-kde")
        })
}

/// True when the binary is a self-contained build (no env var pointing at a
/// real resource tree, e.g. a checkout or the Nix package wrapper). In that case
/// the embedded copies are the source of truth.
pub fn is_self_contained() -> bool {
    std::env::var_os("FANZYZONES_KDE_KWIN_SCRIPT_DIR").is_none()
}

/// Unpack the embedded resources into resource_root(), overwriting prior copies
/// so an upgraded binary refreshes them.
pub fn extract_all() -> Result<()> {
    let root = resource_root();
    for (dir, name) in [
        (&KWIN_SCRIPT, "kwin-script"),
        (&PLASMOID, "plasma-applet"),
        (&ICONS, "icons"),
    ] {
        let target = root.join(name);
        // Clear stale files from previous versions before re-extracting.
        if target.exists() {
            std::fs::remove_dir_all(&target)
                .with_context(|| format!("clear {}", target.display()))?;
        }
        std::fs::create_dir_all(&target)
            .with_context(|| format!("create {}", target.display()))?;
        dir.extract(&target)
            .with_context(|| format!("extract embedded resources to {}", target.display()))?;
    }
    Ok(())
}
