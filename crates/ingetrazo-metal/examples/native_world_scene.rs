//! Borrowed Scene -> production Metal prototype/instance consumer, no Scene clone.
#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("Native Metal Scene acceptance requires macOS");
    std::process::exit(2);
}
#[cfg(target_os = "macos")]
fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
#[cfg(target_os = "macos")]
fn run() -> Result<(), String> {
    objc2::rc::autoreleasepool(|_| {
        use glam::{DMat4, DVec3};
        use ingetrazo_core::{camera::OrbitCamera, groups::Group, mesh::Mesh, scene::Scene};
        use ingetrazo_metal::{
            world::World,
            world_scene::{Adapter, Revisions},
            NativeMetal,
        };
        use ingetrazo_render::StyleConfig;
        let out = std::path::PathBuf::from(std::env::args().nth(1).ok_or("Output required")?);
        std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
        let triangle = |color: [f64; 3]| {
            let mut m = Mesh::new();
            let ids = [
                DVec3::new(-0.6, -0.6, 0.),
                DVec3::new(0.6, -0.6, 0.),
                DVec3::new(0., 0.6, 0.),
            ]
            .map(|p| m.add_vertex(p));
            let f = m.add_face(ids.to_vec(), vec![]).expect("Valid triangle");
            m.faces[f]
                .attrs
                .insert("color".into(), serde_json::json!(color));
            m
        };
        let mut a = Group::with_mesh("A1", triangle([0., 1., 0.]));
        a.mesh_share = Some(a.id);
        a.component_definition = Some(a.id);
        a.native_document = serde_json::json!({"proto":0,"xform":DMat4::IDENTITY.to_cols_array()});
        let mut b = Group::with_mesh("B", triangle([1., 0., 0.]));
        b.mesh_share = Some(b.id);
        b.component_definition = Some(b.id);
        b.native_document = serde_json::json!({"proto":1,"xform":DMat4::IDENTITY.to_cols_array()});
        let a2 = a.duplicate_instance();
        let mut scene = Scene::new();
        scene.groups = vec![a, b, a2];
        let mut rev = Revisions {
            geometry: 1,
            paint: 1,
            style: 1,
            layers: 1,
            topology: 1,
        };
        let mut style = StyleConfig::default();
        style.show_edges = false;
        style.show_profiles = false;
        style.show_grid = false;
        style.show_sky = false;
        style.background_color = [0., 0., 1., 1.];
        let camera = OrbitCamera::default();
        let metal = NativeMetal::new()?;
        let mut world = World::default();
        let mut adapter = Adapter::default();
        let identity = DMat4::IDENTITY.as_mat4().to_cols_array_2d();
        let read = |frame: std::sync::Arc<ingetrazo_metal::world::Prepared>| {
            frame
                .submit_diagnostic(&metal)?
                .readback(std::time::Duration::from_secs(5))
        };
        let save = |name: &str, bytes: Vec<u8>| {
            std::fs::write(out.join(format!("{name}.rgba")), bytes).map_err(|e| e.to_string())
        };
        let first = world
            .prepare_scene_placements(
                &metal,
                &mut adapter,
                &scene,
                rev,
                &camera,
                &style,
                identity,
                (64, 64),
            )?
            .ok_or("Scene frame unavailable")?;
        assert_eq!(first.instance_run_count(), 2);
        save("scene-source-global", read(first)?)?;
        let before = world.stats;
        let builds = adapter.catalog_builds;
        let cpu_before = adapter.retained_payload_bytes();
        let mut cam = identity;
        cam[3][0] = 0.05;
        save(
            "scene-camera-only",
            read(
                world
                    .prepare_scene_placements(
                        &metal,
                        &mut adapter,
                        &scene,
                        rev,
                        &camera,
                        &style,
                        cam,
                        (64, 64),
                    )?
                    .ok_or("Camera frame unavailable")?,
            )?,
        )?;
        assert_eq!(
            world.stats.instance_matrix_upload_bytes,
            before.instance_matrix_upload_bytes
        );
        assert_eq!(adapter.catalog_builds, builds);
        scene.groups[2].transform = DMat4::from_translation(DVec3::X * 0.6);
        save(
            "scene-single-placement-move",
            read(
                world
                    .prepare_scene_placements(
                        &metal,
                        &mut adapter,
                        &scene,
                        rev,
                        &camera,
                        &style,
                        identity,
                        (64, 64),
                    )?
                    .ok_or("Move frame unavailable")?,
            )?,
        )?;
        let moved = world.stats;
        let cpu_after = adapter.retained_payload_bytes();
        assert_eq!(cpu_before, cpu_after);
        assert_eq!(
            moved.instance_matrix_upload_bytes - before.instance_matrix_upload_bytes,
            64
        );
        assert_eq!(moved.geometry_uploads, before.geometry_uploads);
        assert_eq!(moved.draw_order_builds, before.draw_order_builds);
        assert_eq!(adapter.catalog_builds, builds);
        let retained = world
            .prepare_scene_placements(
                &metal,
                &mut adapter,
                &scene,
                rev,
                &camera,
                &style,
                identity,
                (64, 64),
            )?
            .ok_or("Retained frame unavailable")?;
        scene.groups[1].native_document["material"] = serde_json::json!({"opacity":0.5});
        rev.paint += 1;
        let refused = world
            .prepare_scene_placements(
                &metal,
                &mut adapter,
                &scene,
                rev,
                &camera,
                &style,
                identity,
                (64, 64),
            )
            .err()
            .ok_or("Alpha unexpectedly accepted")?;
        assert!(refused.contains("translucent"));
        assert_eq!(adapter.catalog_builds, builds);
        save("scene-refused-alpha-retains-old", read(retained)?)?;
        drop(adapter);
        drop(world);
        let budget = metal.budget.lock().map_err(|_| "Budget poisoned")?;
        let committed: u64 = budget.heaps.iter().map(|h| h.committed).sum();
        let reserved: u64 = budget.heaps.iter().map(|h| h.reserved).sum();
        assert_eq!((committed, reserved), (0, 0));
        std::fs::write(out.join("report.json"),format!("{{\"passed\":true,\"catalog_builds\":{builds},\"warm_geometry_uploads\":{},\"single_model_upload_bytes\":{},\"camera_only_upload_bytes\":0,\"cpu_bytes_before_warm\":{cpu_before},\"cpu_bytes_after_warm\":{cpu_after},\"draw_order_builds\":{},\"ledger_committed\":{committed},\"ledger_reserved\":{reserved},\"alpha_refusal\":{}}}\n",moved.geometry_uploads,moved.instance_matrix_upload_bytes-before.instance_matrix_upload_bytes,moved.draw_order_builds,serde_json::to_string(&refused).unwrap())).map_err(|e|e.to_string())?;
        Ok(())
    })
}
