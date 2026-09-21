fn main() {
    println!("cargo:rerun-if-changed=target/app-icons/app.ico");
    #[cfg(windows)]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        winresource::WindowsResource::new()
            .set_icon("target/app-icons/app.ico")
            .compile()
            .expect("compile Windows icon resource; run scripts/generate_icon_assets.ps1 first");
    }
}
