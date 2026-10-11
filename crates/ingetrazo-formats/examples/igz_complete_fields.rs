use glam::{DMat4, DVec3};
use ingetrazo_formats::igz::{load_igz, save_igz};
use serde_json::json;
use std::path::Path;

fn main() {
    let args: Vec<_> = std::env::args().collect();
    let input = Path::new(&args[1]);
    let output = Path::new(&args[2]);
    std::fs::create_dir_all(output).unwrap();
    for kind in ["no-op", "loose-edit", "rename-parent", "move-child", "remove-copy", "paint"] {
        let mut scene = load_igz(input).unwrap();
        match kind {
            "loose-edit" => {
                let a = scene.mesh.add_vertex(DVec3::new(12., 0., 0.));
                let b = scene.mesh.add_vertex(DVec3::new(13., 1., 0.));
                scene.mesh.add_edge(a, b).unwrap();
                let ring = [(10., 0.), (11., 0.), (10., 1.)]
                    .map(|(x, y)| scene.mesh.add_vertex(DVec3::new(x, y, 0.)));
                scene.mesh.add_face(ring.to_vec(), vec![]).unwrap();
            }
            "rename-parent" => scene.groups[0].name = "Renamed 日本".into(),
            "move-child" => scene.groups[0].children[0].transform *= DMat4::from_translation(DVec3::new(0.125, 0.25, 0.5)),
            "remove-copy" => { scene.groups.pop(); }
            "paint" => {
                let face = scene.mesh.faces.iter_mut().next().unwrap();
                face.attrs.insert("color".into(), json!([0.125, 0.25, 0.75]));
                face.attrs.get_mut("back").unwrap()["opacity"] = json!(0.625);
            }
            _ => {}
        }
        let path = output.join(format!("{kind}.igz"));
        save_igz(&scene, &path).unwrap();
        let reopened = load_igz(&path).unwrap();
        assert_eq!(scene.groups.len(), reopened.groups.len());
        println!("{}", path.display());
    }
}
