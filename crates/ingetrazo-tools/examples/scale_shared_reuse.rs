// SPDX-License-Identifier: GPL-3.0-or-later
#[path = "../src/scale_tool/grips.rs"]
mod grips;
#[path = "../src/scale_tool/targets.rs"]
mod targets;
use glam::DVec3;
use ingetrazo_core::{
    groups::Group,
    image_plane::ScaleMatrix,
    mesh::Mesh,
    scene::{Scene, SelectionItem},
};
use std::{
    hash::{Hash, Hasher},
    time::Instant,
};
fn main() {
    let mut scene = Scene::new();
    let mut mesh = Mesh::new();
    for i in 0..256 {
        mesh.add_vertex(DVec3::new((i % 16) as f64 / 7., (i / 16) as f64 / 7., 0.));
    }
    for y in 0..15 {
        for x in 0..15 {
            let a = y * 16 + x;
            let id = mesh
                .add_face(vec![a, a + 1, a + 17, a + 16], vec![])
                .unwrap();
            mesh.faces[id].attrs.insert(
                "texture".into(),
                serde_json::json!({"path":"grid","uvw":[0.25,0,0,0.1,0,0.5,0,0.2],"vendor":id}),
            );
        }
    }
    let token = uuid::Uuid::new_v4();
    scene.register_mesh_share(token, mesh.clone()).unwrap();
    for i in 0..64 {
        let mut g = Group::with_mesh("shared", mesh.clone());
        g.mesh_share = Some(token);
        scene.groups.push(g);
        scene.select(SelectionItem::Group(i));
    }
    let capture = Instant::now();
    let mut targets = targets::Targets::capture(&scene);
    let capture_seconds = capture.elapsed().as_secs_f64();
    let start = Instant::now();
    for i in 0..4 {
        let matrix = ScaleMatrix::new(
            DVec3::new(0.125, -0.25, 0.),
            DVec3::new(1. + 0.002 * (i % 3) as f64, 0.997, 1.),
            None,
        );
        assert!(targets.shared_commit_finite(&matrix));
        targets.apply(&mut scene, Some(&matrix), true);
    }
    let seconds = start.elapsed().as_secs_f64();
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    for g in &scene.groups {
        for v in &g.mesh.vertices {
            v.position.to_array().map(f64::to_bits).hash(&mut hash);
        }
        for f in &g.mesh.faces {
            serde_json::to_vec(&f.attrs).unwrap().hash(&mut hash);
        }
    }
    println!(
        "{}",
        serde_json::json!({"owners":65,"selected_commands":64,"vertices_per_payload":256,"textured_faces_per_payload":225,"iterations":4,"capture_seconds":capture_seconds,"commit_seconds":seconds,"state_digest":hash.finish()})
    );
}
