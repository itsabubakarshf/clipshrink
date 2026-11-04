// Embeds the app icon and version info into the Windows exe.
fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        res.set("ProductName", "Video Compressor");
        res.set("FileDescription", "Video Compressor - split and compress videos");
        if let Err(e) = res.compile() {
            // Don't fail the build over cosmetics.
            println!("cargo:warning=could not embed the icon: {e}");
        }
    }
}
