use glam::{DMat4, DVec3};
use ingetrazo_core::{
    camera::OrbitCamera, georef::SceneDatum, groups::Group, layers::Layer, mesh::Mesh,
    presentation::ShadowSettings, render_blender::*, scene::Scene,
};
use serde_json::Value;
use std::path::Path;
fn scene(kind: &str) -> Scene {
    let mut s = Scene::new();
    match kind {
        "empty" => {}
        "orphan" => {
            s.mesh.add_vertex(DVec3::new(1000., 0., -99.));
        }
        "loose-edge" | "hidden-layer" => {
            let a = s.mesh.add_vertex(DVec3::new(-25., 0., -3.));
            let b = s.mesh.add_vertex(DVec3::new(25., 0., 2.));
            let id = s.mesh.add_edge(a, b).unwrap();
            s.mesh.edges[id].hidden = true;
            if kind == "hidden-layer" {
                let mut layer = Layer::new("Hidden");
                layer.visible = false;
                s.layers.push(layer);
                s.mesh.edges[id].layer = Some("Hidden".into());
            }
        }
        "hidden-face" => {
            let ids = [
                DVec3::new(0., 0., -2.),
                DVec3::new(40., 0., -2.),
                DVec3::new(0., 50., -2.),
            ]
            .map(|p| s.mesh.add_vertex(p));
            let f = s.mesh.add_face(ids.to_vec(), vec![]).unwrap();
            s.mesh.faces[f].attrs.insert("hidden".into(), true.into());
        }
        _ => {
            let mut mesh = Mesh::new();
            let ids = [
                DVec3::new(0., 0., -4.),
                DVec3::new(40., 0., -4.),
                DVec3::new(0., 50., -2.),
            ]
            .map(|p| mesh.add_vertex(p));
            mesh.add_face(ids.to_vec(), vec![]).unwrap();
            let mut leaf = Group::with_mesh("Leaf", mesh);
            leaf.transform = DMat4::from_translation(DVec3::new(-4., 8., 1.))
                * DMat4::from_scale(DVec3::new(2., 1., 0.5));
            let mut mid = Group::new("Mid");
            mid.transform = DMat4::from_translation(DVec3::new(0.125, 0.25, -3.));
            mid.hidden = kind == "hidden-middle";
            mid.billboard = kind == "billboard-middle";
            mid.children.push(leaf);
            let mut parent = Group::new("Parent");
            parent.transform = DMat4::from_translation(DVec3::new(5., -8., 2.));
            parent.hidden = kind == "hidden-root";
            parent.children.push(mid);
            s.groups.push(parent);
        }
    };
    s
}
fn main() {
    let input: Value = serde_json::from_str(include_str!(
        "../../../docs/audits/blender-core-config/cases.json"
    ))
    .unwrap();
    let mut jobs = vec![];
    for c in input["cases"].as_array().unwrap() {
        let mut s = scene(c["scene"].as_str().unwrap());
        s.shadows = ShadowSettings::from_json(&c["shadow"]);
        if !c["datum"].is_null() {
            s.geo_datum = Some(SceneDatum::new(
                c["datum"]["lat"].as_f64().unwrap(),
                c["datum"]["lon"].as_f64().unwrap(),
                0.,
                c["datum"]["north"].as_f64().unwrap(),
            ));
        }
        let camera: OrbitCamera = serde_json::from_value(c["camera"].clone()).unwrap();
        validate_camera(&camera).unwrap();
        let mut raw_lights = c["lights"].clone();
        for light in raw_lights.as_array_mut().unwrap() {
            light["kind"] = if light["kind"] == "spot" {
                "Spot".into()
            } else {
                "Point".into()
            };
        }
        let lights: Vec<BlenderLight> = serde_json::from_value(raw_lights).unwrap();
        let engine = if c["engine"] == "cycles" {
            BlenderEngine::Cycles
        } else {
            BlenderEngine::Eevee
        };
        let quality = match c["quality"].as_u64().unwrap() {
            0 => RenderQuality::Draft,
            1 => RenderQuality::Medium,
            _ => RenderQuality::High,
        };
        let ambience = match c["ambience"].as_str().unwrap() {
            "day" => Ambience::Day,
            "night" => Ambience::Night,
            _ => Ambience::Overcast,
        };
        jobs.push(serde_json::json!({"name":c["name"],"job":build_job(&s,&camera,Path::new("model.glb"),Path::new("render.png"),None,engine,quality,320,240,ambience,c["sun_scale"].as_f64().unwrap(),c["ground"].as_bool().unwrap(),&lights,c["year"].as_i64().unwrap()as i32)}));
    }
    let env: Vec<_> = input["environments"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            let pairs = c["values"]
                .as_object()
                .unwrap()
                .iter()
                .map(|(k, v)| (k.clone(), v.as_str().unwrap().to_owned()))
                .collect();
            clean_env_values(pairs, c["frozen"].as_bool().unwrap())
                .into_iter()
                .collect::<std::collections::BTreeMap<_, _>>()
        })
        .collect();
    let cache: Vec<_> = input["cache"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            work_base_values(
                Path::new(c["home"].as_str().unwrap()),
                c["xdg"].as_str().map(Path::new),
                c["flatpak"].as_bool().unwrap(),
            )
        })
        .collect();
    let mut refused = vec![];
    for target in [DVec3::new(0.1, 0., 0.), DVec3::new(f64::INFINITY, 0., 0.)] {
        let mut c = OrbitCamera::new();
        c.target = target;
        refused.push(validate_camera(&c).is_err());
    }
    let mut c = OrbitCamera::new();
    c.distance = 1e300;
    refused.push(validate_camera(&c).is_err());
    println!(
        "{}",
        serde_json::json!({"jobs":jobs,"environments":env,"cache":cache,"refused":refused})
    );
}
