//! Separate-process native COLLADA decoder for independent original-runtime checks.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args=std::env::args().skip(1);
    let input=args.next().ok_or("input DAE path required")?;
    let output=args.next().ok_or("output IGZ path required")?;
    let scene=ingetrazo_formats::load_file(std::path::Path::new(&input))?;
    ingetrazo_formats::save_igz(&scene,std::path::Path::new(&output))?;
    println!("{}",serde_json::json!({"groups":scene.groups.len(),"embedded_images":scene.embedded_textures.len()}));
    Ok(())
}
