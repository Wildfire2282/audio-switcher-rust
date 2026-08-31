fn main() {
    embed_manifest::embed_manifest_file("audio-switcher-rust.manifest").unwrap();
    // Embed program icon for Explorer / Task Manager / Alt-Tab
    let mut res = winres::WindowsResource::new();
    res.set_icon("icons/app.ico");
    // Keep manifest handling in winres from interfering; we already embedded via embed_manifest
    // winres will still generate a resource with the icon.
    res.compile().unwrap();
}
