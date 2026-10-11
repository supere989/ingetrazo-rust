//! Actual default-style Metal consumer. Runtime execution requires macOS.
#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("Native Metal required");
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
        use glam::DVec3;
        use ingetrazo_core::{camera::OrbitCamera, scene::Scene};
        use ingetrazo_metal::{
            world::World,
            world_scene::{Adapter, Revisions},
            NativeMetal,
        };
        use ingetrazo_render::StyleConfig;
        let out = std::path::PathBuf::from(std::env::args().nth(1).ok_or("Output required")?);
        std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
        let metal = NativeMetal::new()?;
        let mut world = World::default();
        world.set_sample_count(&metal, 4)?;
        let mut adapter = Adapter::default();
        let mut scene = Scene::new();
        let camera = OrbitCamera::default();
        let mut style = StyleConfig::default();
        let mut rev = Revisions {
            geometry: 1,
            paint: 1,
            style: 1,
            layers: 1,
            topology: 1,
        };
        let matrix = (camera.projection_matrix() * camera.view_matrix())
            .as_mat4()
            .to_cols_array_2d();
        let read = |frame: std::sync::Arc<ingetrazo_metal::world::Prepared>| {
            frame
                .submit_diagnostic(&metal)?
                .readback(std::time::Duration::from_secs(5))
        };
        let prepare = |world: &mut World,
                       adapter: &mut Adapter,
                       scene: &Scene,
                       rev: Revisions,
                       style: &StyleConfig,
                       eye: DVec3|
         -> Result<std::sync::Arc<ingetrazo_metal::world::Prepared>, String> {
            world
                .prepare_scene_preview(
                    &metal,
                    adapter,
                    scene,
                    rev,
                    Some(1),
                    &camera,
                    style,
                    matrix,
                    (64, 64),
                    None,
                    Some(eye),
                )?
                .ok_or("Default frame unavailable".into())
        };
        // Empty startup still renders default sky, without fake geometry or textures.
        let sky = read(prepare(
            &mut world,
            &mut adapter,
            &scene,
            rev,
            &style,
            DVec3::new(0., -3., 1.),
        )?)?;
        assert!(sky.chunks_exact(4).any(|p| p != &sky[..4]));
        std::fs::write(out.join("startup.rgba"), &sky).map_err(|e| e.to_string())?;
        let ids = [
            DVec3::new(-0.6, 0., 0.),
            DVec3::new(0.6, 0., 0.),
            DVec3::new(0., 0.6, 0.),
            DVec3::new(0., 0., 0.6),
        ]
        .map(|p| scene.mesh.add_vertex(p));
        scene
            .mesh
            .add_face(vec![ids[0], ids[1], ids[2]], vec![])
            .ok_or("Invalid folded face")?;
        scene
            .mesh
            .add_face(vec![ids[1], ids[0], ids[3]], vec![])
            .ok_or("Invalid folded face")?;
        for e in &mut scene.mesh.edges {
            e.soft = e.faces.len() == 2;
        }
        rev.geometry += 1;
        rev.topology += 1;
        let first = read(prepare(
            &mut world,
            &mut adapter,
            &scene,
            rev,
            &style,
            DVec3::new(0., 2., 2.),
        )?)?;
        let before = world.stats;
        let warm = read(prepare(
            &mut world,
            &mut adapter,
            &scene,
            rev,
            &style,
            DVec3::new(0., 2., 2.),
        )?)?;
        assert_eq!(first, warm);
        assert_eq!(world.stats.profile_builds, before.profile_builds);
        assert_eq!(world.stats.profile_uploads, before.profile_uploads);
        assert_eq!(world.stats.geometry_uploads, before.geometry_uploads);
        let camera_changed = read(prepare(
            &mut world,
            &mut adapter,
            &scene,
            rev,
            &style,
            DVec3::new(0., -2., 2.),
        )?)?;
        assert_eq!(world.stats.profile_builds, before.profile_builds);
        assert_eq!(world.stats.geometry_uploads, before.geometry_uploads);
        assert!(world.stats.profile_uploads > before.profile_uploads);
        style.show_profiles = false;
        rev.style += 1;
        let off = read(prepare(
            &mut world,
            &mut adapter,
            &scene,
            rev,
            &style,
            DVec3::new(0., -2., 2.),
        )?)?;
        style.show_profiles = true;
        rev.style += 1;
        scene.mesh.edges.iter_mut().for_each(|e| e.soft = false);
        rev.topology += 1;
        let edited = read(prepare(
            &mut world,
            &mut adapter,
            &scene,
            rev,
            &style,
            DVec3::new(0., -2., 2.),
        )?)?;
        let accepted = world.stats;
        let retained = prepare(
            &mut world,
            &mut adapter,
            &scene,
            rev,
            &style,
            DVec3::new(0., -2., 2.),
        )?;
        style.show_grid = true;
        let error = prepare(
            &mut world,
            &mut adapter,
            &scene,
            rev,
            &style,
            DVec3::new(0., -2., 2.),
        )
        .err()
        .ok_or("Unsupported grid accepted")?;
        assert!(error.contains("grid"));
        assert_eq!(world.stats.geometry_uploads, accepted.geometry_uploads);
        assert_eq!(edited, read(retained)?);
        for (name, bytes) in [
            ("default", first),
            ("camera", camera_changed),
            ("profiles-off", off),
            ("soft-edited", edited),
        ] {
            std::fs::write(out.join(format!("{name}.rgba")), bytes).map_err(|e| e.to_string())?;
        }
        drop(adapter);
        drop(world);
        let budget = metal.budget.lock().map_err(|_| "Budget poisoned")?;
        let committed: u64 = budget.heaps.iter().map(|h| h.committed).sum();
        let reserved: u64 = budget.heaps.iter().map(|h| h.reserved).sum();
        assert_eq!((committed, reserved), (0, 0));
        std::fs::write(out.join("report.json"),serde_json::to_vec_pretty(&serde_json::json!({"passed":true,"extent":[64,64],"samples":4,"default_style":true,"warm_extra_profile_uploads":0,"warm_extra_geometry_uploads":0,"camera_extra_base_uploads":0,"profile_builds":accepted.profile_builds,"profile_uploads":accepted.profile_uploads,"profile_upload_bytes":accepted.profile_upload_bytes,"unsupported_grid_refusal":error,"ledger_committed":committed,"ledger_reserved":reserved})).unwrap()).map_err(|e|e.to_string())?;
        Ok(())
    })
}
