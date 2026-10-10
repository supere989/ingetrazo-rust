use ingetrazo_formats::skp;
use std::path::Path;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().collect::<Vec<_>>();
    if args.len() != 4 {
        return Err("INPUTskp OUTPUTigz OUTPUTpayloadjson".into());
    }
    let payload = skp::parse_skp(Path::new(&args[1]))?;
    std::fs::write(&args[3], serde_json::to_vec_pretty(&payload)?)?;
    let scene = skp::scene_from_payload(&payload)?;
    ingetrazo_formats::save_igz(&scene, Path::new(&args[2]))?;
    println!(
        "SKP Rust import PASS groups={} faces={} edges={}",
        scene.groups.len(),
        scene
            .groups
            .iter()
            .map(|g| g.mesh.faces.len())
            .sum::<usize>(),
        scene
            .groups
            .iter()
            .map(|g| g.mesh.edges.len())
            .sum::<usize>()
    );
    Ok(())
}
