//! Build script: single-source versioning from `CARGO_PKG_VERSION`.
#![allow(missing_docs)]
fn main() {
    // Non-Windows no-op guard: keeps Linux `cargo check` green.
    if std::env::var("CARGO_CFG_WINDOWS").is_err() {
        return;
    }
    println!("cargo:rerun-if-changed=audio-switcher.manifest");
    println!("cargo:rerun-if-changed=icons/app.ico");
    println!("cargo:rerun-if-changed=build.rs");

    // Manifest assemblyIdentity: first three Cargo segments + `.0`,
    // prerelease suffixes (`-rc.N`) stripped. Humans edit Cargo.toml only.
    let cargo_version = std::env::var("CARGO_PKG_VERSION").expect("CARGO_PKG_VERSION set by cargo");
    let win_version = manifest_version(&cargo_version);
    let source =
        std::fs::read_to_string("audio-switcher.manifest").expect("manifest source readable");
    let generated_xml = source.replace(
        r#"version="0.0.0.0""#,
        &format!(r#"version="{win_version}""#),
    );
    assert!(
        generated_xml != source,
        "manifest placeholder version 0.0.0.0 missing"
    );
    let out_path = std::path::Path::new(&std::env::var("OUT_DIR").expect("OUT_DIR set by cargo"))
        .join("audio-switcher.manifest");
    std::fs::write(&out_path, generated_xml).expect("generated manifest writable");
    embed_manifest::embed_manifest_file(&out_path).expect("manifest embed failed");

    // Program icon for Explorer / Task Manager / Alt-Tab. VERSIONINFO
    // (FILEVERSION/PRODUCTVERSION) is derived from CARGO_PKG_VERSION by
    // winres defaults; the manifest is embedded above, not via winres.
    let mut res = winres::WindowsResource::new();
    res.set_icon("icons/app.ico");
    res.compile().expect("winres compile failed");
}

/// `0.3.0` → `0.3.0.0`; `1.2.3-rc.1` → `1.2.3.0`.
fn manifest_version(cargo: &str) -> String {
    let base = cargo.split('-').next().unwrap_or(cargo);
    let mut parts: Vec<&str> = base.split('.').collect();
    while parts.len() < 3 {
        parts.push("0");
    }
    format!("{}.{}.{}.0", parts[0], parts[1], parts[2])
}
