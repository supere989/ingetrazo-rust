fn main() {
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
