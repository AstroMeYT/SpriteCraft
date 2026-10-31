// Embeds the application icon into the Windows executable (only when targeting Windows).
fn main() {
    println!("cargo:rerun-if-changed=assets/spritecraft.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/spritecraft.ico");
        res.set("ProductName", "SpriteCraft");
        res.set("FileDescription", "SpriteCraft - pixel art and animation studio");
        if let Err(e) = res.compile() {
            println!("cargo:warning=could not embed Windows icon: {e}");
        }
    }
}
