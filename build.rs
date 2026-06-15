use cxx_qt_build::{CxxQtBuilder, QmlModule};
use std::env;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::symlink;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    configure_split_nix_qt_tools();

    CxxQtBuilder::new_qml_module(QmlModule::new("FanzyZones"))
        .qt_module("Gui")
        .qt_module("Network")
        .qt_module("Quick")
        .files(["src/backend.rs"])
        .build();
}

fn configure_split_nix_qt_tools() {
    let Some(declarative_libexec) = find_declarative_libexec() else {
        return;
    };
    if !declarative_libexec.join("qmltyperegistrar").exists() {
        return;
    }

    let Some(real_qmake) = env::var_os("QMAKE")
        .map(PathBuf::from)
        .or_else(|| find_on_path("qmake"))
    else {
        return;
    };
    let Some(base_libexec) = query_qmake_libexec(&real_qmake) else {
        return;
    };
    let Some(base_headers) = query_qmake_path(
        &real_qmake,
        &[
            "QT_INSTALL_HEADERS/get",
            "QT_INSTALL_HEADERS",
            "QT_HOST_HEADERS/get",
            "QT_HOST_HEADERS",
        ],
    ) else {
        return;
    };
    let Some(declarative_headers) = declarative_libexec
        .parent()
        .map(|path| path.join("include"))
        .filter(|path| path.exists())
    else {
        return;
    };
    let Some(out_dir) = env::var_os("OUT_DIR").map(PathBuf::from) else {
        return;
    };

    let combined_libexec = out_dir.join("fanzyzones-qt-libexec");
    if fs::create_dir_all(&combined_libexec).is_err() {
        return;
    }
    link_tool(&combined_libexec, &base_libexec, "moc");
    link_tool(&combined_libexec, &base_libexec, "rcc");
    link_tool(&combined_libexec, &declarative_libexec, "qmlcachegen");
    link_tool(&combined_libexec, &declarative_libexec, "qmltyperegistrar");

    let combined_headers = out_dir.join("fanzyzones-qt-include");
    if fs::create_dir_all(&combined_headers).is_err() {
        return;
    }
    link_directory_entries(&combined_headers, &base_headers);
    link_directory_entries(&combined_headers, &declarative_headers);

    let qmake_wrapper = out_dir.join("fanzyzones-qmake");
    let script = format!(
        "#!/bin/sh\nif [ \"$1\" = \"-query\" ]; then\n  case \"$2\" in\n    QT_HOST_LIBEXECS|get|QT_HOST_LIBEXECS/get|QT_INSTALL_LIBEXECS|QT_INSTALL_LIBEXECS/get)\n      printf '%s\\n' {}\n      exit 0\n      ;;\n    QT_INSTALL_HEADERS|get|QT_INSTALL_HEADERS/get|QT_HOST_HEADERS|QT_HOST_HEADERS/get)\n      printf '%s\\n' {}\n      exit 0\n      ;;\n  esac\nfi\nexec {} \"$@\"\n",
        shell_quote(&combined_libexec),
        shell_quote(&combined_headers),
        shell_quote(&real_qmake),
    );
    if fs::write(&qmake_wrapper, script).is_ok() {
        #[cfg(unix)]
        {
            let _ = fs::set_permissions(&qmake_wrapper, fs::Permissions::from_mode(0o755));
        }
        env::set_var("QMAKE", qmake_wrapper);
    }
}

fn find_declarative_libexec() -> Option<PathBuf> {
    if let Some(path) = env::var_os("QT_DECLARATIVE_LIBEXEC").map(PathBuf::from) {
        return Some(path);
    }

    env::var_os("PATH")?
        .to_string_lossy()
        .split(':')
        .map(PathBuf::from)
        .map(|path| path.join("../libexec"))
        .find(|path| path.join("qmltyperegistrar").exists())
}

fn find_on_path(binary: &str) -> Option<PathBuf> {
    env::var_os("PATH")?
        .to_string_lossy()
        .split(':')
        .map(PathBuf::from)
        .map(|path| path.join(binary))
        .find(|path| path.exists())
}

fn query_qmake_libexec(qmake: &Path) -> Option<PathBuf> {
    query_qmake_path(
        qmake,
        &[
            "QT_HOST_LIBEXECS/get",
            "QT_HOST_LIBEXECS",
            "QT_INSTALL_LIBEXECS/get",
            "QT_INSTALL_LIBEXECS",
        ],
    )
    .filter(|path| path.join("moc").exists() || path.join("rcc").exists())
}

fn query_qmake_path(qmake: &Path, keys: &[&str]) -> Option<PathBuf> {
    keys.iter()
        .filter_map(|key| {
            let output = Command::new(qmake).arg("-query").arg(key).output().ok()?;
            output.status.success().then_some(output)
        })
        .filter_map(|output| {
            let value = String::from_utf8(output.stdout).ok()?;
            let path = PathBuf::from(value.trim());
            path.exists().then_some(path)
        })
        .next()
}

fn link_directory_entries(target_dir: &Path, source_dir: &Path) {
    let Ok(entries) = fs::read_dir(source_dir) else {
        return;
    };

    for entry in entries.flatten() {
        let source = entry.path();
        let target = target_dir.join(entry.file_name());
        if target.exists() {
            continue;
        }

        #[cfg(unix)]
        if symlink(&source, &target).is_ok() {
            continue;
        }

        if source.is_dir() {
            let _ = fs::create_dir_all(target);
        } else {
            let _ = fs::copy(source, target);
        }
    }
}

fn link_tool(combined_libexec: &Path, source_dir: &Path, tool: &str) {
    let source = source_dir.join(tool);
    if !source.exists() {
        return;
    }

    let target = combined_libexec.join(tool);
    let _ = fs::remove_file(&target);
    #[cfg(unix)]
    if symlink(&source, &target).is_ok() {
        return;
    }
    let _ = fs::copy(source, target);
}

fn shell_quote(path: &Path) -> String {
    format!("'{}'", path.to_string_lossy().replace('\'', "'\\''"))
}
