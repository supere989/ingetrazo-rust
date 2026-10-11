use glam::DVec3;
use ingetrazo_core::image_plane::ImagePlane;
use ingetrazo_formats::igz::{load_igz, save_igz};
use serde_json::json;
use std::path::Path;
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let cases: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&args[1]).unwrap()).unwrap();
    std::fs::create_dir_all(&args[2]).unwrap();
    for c in cases.as_array().unwrap() {
        let mut scene = load_igz(Path::new(c["input"].as_str().unwrap())).unwrap();
        let assets = Path::new(c["assets"].as_str().unwrap());
        let a = scene.mesh.add_vertex(DVec3::new(1000., 0., 0.));
        let b = scene.mesh.add_vertex(DVec3::new(1001., 1., 0.));
        let edge = scene.mesh.add_edge(a, b).unwrap();
        scene.mesh.edges[edge]
            .attrs
            .insert("layer".into(), json!(12));
        scene.mesh.edges[edge]
            .attrs
            .insert("curve".into(), json!(-3));
        let ring = [(1100., 0.), (1103., 0.), (1103., 2.), (1100., 2.)]
            .map(|(x, y)| scene.mesh.add_vertex(DVec3::new(x, y, 0.)));
        let face = scene.mesh.add_face(ring.to_vec(), vec![]).unwrap();
        let attrs = json!({"color":[0.125,0.5,0.75],"opacity":0.625,"layer":7,
            "texture":{"path":assets.join("paint.png"),"base":assets.join("base.png"),"sw":2,"sh":3,"rot":17},
            "back":{"color":[0.75,0.25,0.125],"texture":{"path":assets.join("back.png"),"sw":1,"sh":1}}});
        scene.mesh.faces[face]
            .attrs
            .extend(attrs.as_object().unwrap().clone());
        if let Some(g) = scene.groups.first_mut() {
            g.name = format!("Edited {}", g.name);
        }
        let mut plane = ImagePlane::new(
            assets.join("plane.png").to_string_lossy(),
            DVec3::new(1200., 0., 0.),
            DVec3::new(4., 0., 0.),
            DVec3::new(0., 2., 0.),
        );
        plane.name = "Fresh reference".into();
        plane.opacity = 0.375;
        scene.image_planes.push(plane);
        let path = Path::new(&args[2]).join(format!("{}.igz", c["name"].as_str().unwrap()));
        save_igz(&scene, &path).unwrap();
        let reopened = load_igz(&path).unwrap();
        fn ids(groups: &[ingetrazo_core::groups::Group], out: &mut Vec<uuid::Uuid>) {
            for g in groups {
                out.push(g.id);
                ids(&g.children, out);
            }
        }
        let mut before = vec![];
        let mut after = vec![];
        ids(&scene.groups, &mut before);
        ids(&reopened.groups, &mut after);
        assert_eq!(
            before, after,
            "Runtime group identities must survive actual save/reopen"
        );
        println!("{}", path.display());
    }
}
