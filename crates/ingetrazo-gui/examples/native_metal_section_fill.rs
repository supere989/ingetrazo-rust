//! Production Scene/World stencil fill against retained original section recipes.
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
    ingetrazo_metal::autoreleasepool(|_| {
        use glam::DVec3;
        use ingetrazo_core::{
            camera::{OrbitCamera, StandardView},
            scene::Scene,
            section::SectionPlane,
        };
        use ingetrazo_metal::{
            world::World,
            world_scene::{Adapter, Revisions},
            NativeMetal,
        };
        use ingetrazo_render::StyleConfig;
        let out = std::path::PathBuf::from(std::env::args().nth(1).ok_or("Output required")?);
        std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
        let metal = NativeMetal::new()?;
        let sample_diagnostics = ingetrazo_metal::annotation::sample_diagnostics(&metal);
        let mut rows = vec![];
        for name in [
            "closed",
            "shared-slab",
            "shared-slab-reversed",
            "transparent",
            "duplicate",
            "coplanar-backface",
            "closed-iso",
            "shared-slab-iso",
            "shared-slab-reversed-iso",
            "closed-front",
        ] {
            let mut scene = Scene::new();
            let mut layers = vec![];
            for z in [0., 1., 2.] {
                layers.push(
                    [[0., 0., z], [1., 0., z], [1., 1., z], [0., 1., z]]
                        .map(|p| scene.mesh.add_vertex(DVec3::from_array(p))),
                );
            }
            scene
                .mesh
                .add_face(layers[0].into_iter().rev().collect(), vec![])
                .ok_or("Source section fixture face insertion refused")?;
            scene
                .mesh
                .add_face(layers[2].to_vec(), vec![])
                .ok_or("Source section fixture face insertion refused")?;
            for i in 0..4 {
                let j = (i + 1) % 4;
                for k in 0..2 {
                    scene
                        .mesh
                        .add_face(
                            vec![
                                layers[k][i],
                                layers[k][j],
                                layers[k + 1][j],
                                layers[k + 1][i],
                            ],
                            vec![],
                        )
                        .ok_or("Source section fixture face insertion refused")?;
                }
            }
            if name.starts_with("shared-slab") {
                let mut ring = layers[1].to_vec();
                if name.contains("reversed") {
                    ring.reverse();
                }
                let id = scene
                    .mesh
                    .add_face(ring, vec![])
                    .ok_or("Source section fixture face insertion refused")?;
                scene.mesh.faces[id].interior = true;
            }
            if name == "transparent" {
                for f in &mut scene.mesh.faces {
                    f.attrs.insert("opacity".into(), serde_json::json!(0.5));
                }
            }
            if name == "duplicate" {
                let faces = scene.mesh.faces.clone();
                for f in faces {
                    scene
                        .mesh
                        .add_face(f.vertices, vec![])
                        .ok_or("Source section fixture face insertion refused")?;
                }
            }
            if name == "coplanar-backface" {
                scene.mesh = Default::default();
                let ids = [[0., 1., 1.5], [1., 1., 1.5], [1., 0., 1.5], [0., 0., 1.5]]
                    .map(|p| scene.mesh.add_vertex(DVec3::from_array(p)));
                scene
                    .mesh
                    .add_face(ids.to_vec(), vec![])
                    .ok_or("Source section fixture face insertion refused")?;
            }
            scene.section_planes.push(SectionPlane::new(
                DVec3::new(0., 0., 1.5),
                DVec3::Z,
                "A",
                "A",
            ));
            scene.show_section_cuts = true;
            scene.show_section_fill = true;
            let mut camera = OrbitCamera::default();
            camera.set_standard_view(if name.ends_with("iso") {
                StandardView::Iso
            } else if name.ends_with("front") {
                StandardView::Front
            } else {
                StandardView::Top
            });
            camera.perspective = false;
            camera.target = DVec3::new(0.5, 0.5, 1.5);
            camera.distance = 5.;
            camera.aspect = 800. / 600.;
            let source =
                ingetrazo_gui::navigation_ray::camera_snapshot(&camera).ok_or("Camera refused")?;
            let mut style = StyleConfig::default();
            style.show_edges = false;
            style.show_profiles = false;
            style.show_sky = false;
            style.show_grid = false;
            style.section_fill_color = [1., 0., 1., 1.];
            let mut world = World::default();
            world.set_sample_count(&metal, 4)?;
            let mut adapter = Adapter::default();
            let rev = Revisions {
                geometry: 1,
                paint: 1,
                style: 1,
                layers: 1,
                topology: 1,
            };
            let prepare = |world: &mut World, adapter: &mut Adapter, scene: &Scene| -> Result<std::sync::Arc<ingetrazo_metal::world::Prepared>,String> {
                world
                    .prepare_scene_preview(
                        &metal,
                        adapter,
                        scene,
                        rev,
                        Some(1),
                        &camera,
                        &style,
                        source.matrix,
                        (800, 600),
                        None,
                        Some(DVec3::from_array(source.eye.map(f64::from))),
                    )?
                    .ok_or("Frame unavailable".into())
            };
            let frame = prepare(&mut world, &mut adapter, &scene)?;
            let programmed_positions = frame.programmed_sample_positions();
            if sample_diagnostics["programmable"] == true {
                if serde_json::to_value(programmed_positions).map_err(|e|e.to_string())?
                    != sample_diagnostics["source_positions_top_origin"] {
                    return Err(format!("Section-only source sample pattern missing: {name}"));
                }
            } else if programmed_positions.is_some() {
                return Err(format!("Unsupported sample positions programmed: {name}"));
            }
            if !frame.section_contours_enabled(){return Err(format!("Contour route missing: {name}"));}
            let bytes = frame
                .submit_diagnostic(&metal)?
                .readback(std::time::Duration::from_secs(5))?;
            std::fs::write(out.join(format!("{name}.rgba")), &bytes).map_err(|e| e.to_string())?;
            drop(frame);
            let before = world.stats;
            let mut repeated = None;
            for _ in 0..3 {
                let frame = prepare(&mut world, &mut adapter, &scene)?;
                let pixels = frame
                    .submit_diagnostic(&metal)?
                    .readback(std::time::Duration::from_secs(5))?;
                if pixels != bytes {
                    return Err(format!("Warm pixels changed: {name}"));
                }
                repeated = Some(pixels);
                drop(frame);
            }
            let after = world.stats;
            if before.target_allocations != after.target_allocations
                || before.section_contour_pipeline_builds!=after.section_contour_pipeline_builds
                || before.geometry_uploads != after.geometry_uploads || before.section_fill_uploads!=after.section_fill_uploads || before.section_fill_upload_bytes!=after.section_fill_upload_bytes
            {
                return Err(format!("Warm allocation changed: {name}"));
            }
            scene.show_section_fill = false;
            let off = prepare(&mut world, &mut adapter, &scene)?;
            if !off.section_contours_enabled(){return Err(format!("Fill-off disabled contours: {name}"));}
            let disabled = off
                .submit_diagnostic(&metal)?
                .readback(std::time::Duration::from_secs(5))?;
            std::fs::write(out.join(format!("{name}-disabled.rgba")), disabled)
                .map_err(|e| e.to_string())?;
            drop(off);
            let fill_off=world.stats;
            scene.show_section_cuts=false;
            let cuts_off=prepare(&mut world,&mut adapter,&scene)?;
            if cuts_off.section_contours_enabled(){return Err(format!("Cuts-off retains contour route: {name}"));}
            let _=cuts_off.submit_diagnostic(&metal)?.readback(std::time::Duration::from_secs(5))?;
            drop(cuts_off);
            let cuts_off_stats=world.stats;
            if cuts_off_stats.section_contour_pipeline_builds!=fill_off.section_contour_pipeline_builds||cuts_off_stats.geometry_uploads!=fill_off.geometry_uploads||cuts_off_stats.target_allocations!=fill_off.target_allocations{return Err(format!("Cuts-off rebuilt geometry/target/pipeline: {name}"));}

            drop(repeated);
            rows.push(serde_json::json!({"case":name,"programmed_sample_positions_top_origin":programmed_positions,"width":800,"height":600,"warm_frames":3,"contours_enabled":true,"fill_off_contours_enabled":true,"cuts_off_contours_enabled":false,"section_contour_pipeline_builds_before":before.section_contour_pipeline_builds,"section_contour_pipeline_builds_after":after.section_contour_pipeline_builds,"fill_off_contour_pipeline_builds":fill_off.section_contour_pipeline_builds,"cuts_off_contour_pipeline_builds":cuts_off_stats.section_contour_pipeline_builds,"target_allocations_before":before.target_allocations,"target_allocations_after":after.target_allocations,"geometry_uploads_before":before.geometry_uploads,"geometry_uploads_after":after.geometry_uploads,"section_fill_uploads_before":before.section_fill_uploads,"section_fill_uploads_after":after.section_fill_uploads,"section_fill_upload_bytes_before":before.section_fill_upload_bytes,"section_fill_upload_bytes_after":after.section_fill_upload_bytes}));
            drop(adapter);
            drop(world);
        }
        std::fs::write(out.join("report.json"),serde_json::to_vec_pretty(&serde_json::json!({"cases":rows,"sample_diagnostics":sample_diagnostics,"scope":"Production native Metal stencil and direct GPU contours; strict all-source RGBA pixels plus fill-off and cuts-off routing/warm controls"})).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
        Ok(())
    })
}
