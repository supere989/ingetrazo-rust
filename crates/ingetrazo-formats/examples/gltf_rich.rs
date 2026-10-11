use std::{fs::File, path::Path};
fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    let scene = ingetrazo_formats::load_igz(Path::new(&args[1])).unwrap();
    let mut out = File::create(&args[2]).unwrap();
    ingetrazo_formats::export_glb(&scene, &mut out).unwrap();
}
