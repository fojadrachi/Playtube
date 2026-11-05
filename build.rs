fn main() {
    let config = slint_build::CompilerConfiguration::new().with_style("fluent-dark".into());
    slint_build::compile_with_config("ui/app.slint", config).expect("Slint-Oberfläche");

    // Icon (Ressource 1, auch für das Tray-Icon) und Versionsinfo der Playtube.exe.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let version = std::env::var("CARGO_PKG_VERSION").unwrap();
        let mut res = winresource::WindowsResource::new();
        res.set_icon_with_id("assets/icon.ico", "1");
        res.set("ProductName", "Playtube");
        res.set("FileDescription", "Playtube");
        res.set("CompanyName", "Fojadrachi");
        res.set("OriginalFilename", "Playtube.exe");
        res.set("InternalName", "Playtube");
        res.set("ProductVersion", &version);
        res.set("FileVersion", &version);
        res.compile().expect("Windows-Ressourcen");
    }
}
