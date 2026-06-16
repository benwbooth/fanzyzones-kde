// Keep every shipped version number on one value. The Rust binary, the Plasma
// applet kpackage, and the KWin script kpackage each carry their own version
// field; KDE's About dialog reads the kpackage ones. Rather than stamp them at
// install time, we keep the source metadata in sync and fail the build if it
// drifts from Cargo.toml, so a forgotten bump can never ship.
use std::env;
use std::fs;

fn main() {
    let version = env::var("CARGO_PKG_VERSION").expect("CARGO_PKG_VERSION is set by cargo");
    println!("cargo:rerun-if-changed=Cargo.toml");

    for path in ["plasma-applet/metadata.json", "kwin-script/metadata.json"] {
        println!("cargo:rerun-if-changed={path}");
        let text = fs::read_to_string(path).unwrap_or_else(|e| panic!("read {path}: {e}"));
        let found = extract_version(&text)
            .unwrap_or_else(|| panic!("no KPlugin.Version field found in {path}"));
        assert_eq!(
            found, version,
            "\n\nversion drift: {path} declares Version {found:?} but Cargo.toml is {version:?}.\n\
             Set both plasma-applet/metadata.json and kwin-script/metadata.json to {version:?}.\n"
        );
    }
}

/// Pull the string value of the first `"Version"` key out of a metadata.json.
fn extract_version(text: &str) -> Option<String> {
    let after_key = &text[text.find("\"Version\"")?..];
    let after_colon = &after_key[after_key.find(':')?+ 1..];
    let start = after_colon.find('"')? + 1;
    let end = after_colon[start..].find('"')? + start;
    Some(after_colon[start..end].to_string())
}
