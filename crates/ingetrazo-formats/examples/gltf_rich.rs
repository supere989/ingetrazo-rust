use std::{fs::File, path::Path};
fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    let scene = ingetrazo_formats::load_igz(Path::new(&args[1])).unwrap();
    let original = scene.clone();
    let mut out = File::create(&args[2]).unwrap();
    if let Some(path) = args.get(3) {
        let data: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let mut camera = ingetrazo_core::camera::OrbitCamera::default();
        camera.target =
            glam::DVec3::from_array(std::array::from_fn(|i| data["target"][i].as_f64().unwrap()));
        camera.distance = data["distance"].as_f64().unwrap();
        camera.yaw = data["yaw"].as_f64().unwrap();
        camera.pitch = data["pitch"].as_f64().unwrap();
        camera.perspective = data["perspective"].as_bool().unwrap();
        if args.get(4).is_some_and(|s| s == "--refusal") {
            let mut sentinel = b"old-output".to_vec();
            let result =
                ingetrazo_formats::gltf::export_glb_with_camera(&scene, &camera, &mut sentinel);
            assert!(result.is_err());
            assert_eq!(sentinel, b"old-output");
            assert_eq!(scene, original);
            println!(
                "{}",
                serde_json::json!({"refused":true,"writer_unchanged":true,"scene_unchanged":true,"reason":result.unwrap_err().to_string()})
            );
            return;
        }
        ingetrazo_formats::gltf::export_glb_with_camera(&scene, &camera, &mut out).unwrap();
    } else {
        ingetrazo_formats::export_glb(&scene, &mut out).unwrap();
    }
    assert_eq!(scene, original, "export mutated input");
}
