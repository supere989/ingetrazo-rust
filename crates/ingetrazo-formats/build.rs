fn main() {
    println!("cargo:rerun-if-env-changed=INGETRAZO_RUST_METADATA_ONLY");
    if std::env::var("INGETRAZO_RUST_METADATA_ONLY").as_deref() == Ok("1") {
        let host = std::env::var("HOST").unwrap();
        let target = std::env::var("TARGET").unwrap();
        assert!(host != target && matches!(target.as_str(), "aarch64-apple-darwin" | "x86_64-apple-darwin"),
            "Metadata-only mode is restricted to cross-target Apple diagnostics");
        println!("cargo:warning=Rust metadata diagnostic only: native JPEG C ABI and linking require native platform acceptance");
        return;
    }
    println!("cargo:rerun-if-changed=src/sheet_jpeg.c");
    let mut build = cc::Build::new();
    build.file("src/sheet_jpeg.c");
    for path in std::env::var("DEP_TURBOJPEG_INCLUDE")
        .expect("Pinned native JPEG headers missing")
        .split(',')
    {
        build.include(path);
    }
    build.compile("ingetrazo_sheet_jpeg");
}
