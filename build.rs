//! Build script — embeds manifest and icon. No public API docs needed.
#![allow(missing_docs)]
fn main() {
    println!("cargo:rerun-if-changed=audio-switcher-rust.manifest");
    println!("cargo:rerun-if-changed=icons/app.ico");
    println!("cargo:rerun-if-changed=build.rs");
    embed_manifest::embed_manifest_file("audio-switcher-rust.manifest")
        .expect("audio-switcher-rust.manifest missing or invalid");
    // Embed program icon for Explorer / Task Manager / Alt-Tab
    let mut res = winres::WindowsResource::new();
    res.set_icon("icons/app.ico");
    // Manifest already embedded via embed_manifest; winres icon only.
    // Note: winres default manifest disabled by not calling set_manifest_file.
    res.compile().expect("winres compile failed");
}
