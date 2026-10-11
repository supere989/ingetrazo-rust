//! Closed-only Intel Adapter / resolve ownership diagnostic.
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
        if sample_diagnostics["supports_family_apple4"] == true || metal.info.unified_memory { return Err("Intel-only diagnostic refuses Apple/unified device".into()); }
        let mut rows = vec![];
        for _name in ["closed"] {
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
            scene.section_planes.push(SectionPlane::new(
                DVec3::new(0., 0., 1.5),
                DVec3::Z,
                "A",
                "A",
            ));
            scene.show_section_cuts = true;
            scene.show_section_fill = true;
            let mut camera = OrbitCamera::default();
            camera.set_standard_view(StandardView::Top);
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

            for reuse in [true,false] { for compute in [false,true] {
                let cell=format!("{}-{}",if reuse{"reused"}else{"fresh"},if compute{"compute"}else{"native"});
                let mut adapter=Adapter::default(); let mut seed=World::default();
                seed.use_native_resolve_reference()?; seed.set_sample_count(&metal,4)?;
                let frame=prepare(&mut seed,&mut adapter,&scene)?;
                if !frame.section_contours_enabled(){return Err("Closed seed contours absent".into());}
                let submission=frame.submit_diagnostic(&metal)?;
                let seed_bytes=submission.readback(std::time::Duration::from_secs(5))?;
                if seed_bytes.len()!=1_920_000{return Err("Seed frame length mismatch".into());}
                std::fs::write(out.join(format!("{cell}-seed.rgba")),&seed_bytes).map_err(|e|e.to_string())?;
                let seed_snapshot=submission.resource_observation();
                drop(submission);drop(frame);drop(seed);
                if !reuse {adapter=Adapter::default();}
                let mut world=World::default();if compute {world.use_native_resolve_reference()?;}world.set_sample_count(&metal,4)?;
                if compute {world.enable_source_resolve_prototype(&metal)?;}
                let mut first:Option<Vec<u8>>=None;
                for iteration in 0..4 {
                    let frame=prepare(&mut world,&mut adapter,&scene)?;
                    if compute && frame.source_resolve_diagnostic()["path"]!="compute" {return Err("Intel compute required; Apple tile outside probe".into());}
                    if !frame.section_contours_enabled(){return Err("Closed candidate contours absent".into());}
                    let submission=frame.submit_diagnostic(&metal)?;
                    let bytes=submission.readback(std::time::Duration::from_secs(5))?;
                    if bytes.len()!=1_920_000{return Err("Candidate frame length mismatch".into());}
                    std::fs::write(out.join(format!("{cell}-{iteration}.rgba")),&bytes).map_err(|e|e.to_string())?;
                    let changed=|a:&[u8],b:&[u8]|a.chunks_exact(4).zip(b.chunks_exact(4)).filter(|(a,b)|a!=b).count();
                    let row=serde_json::json!({"cell":cell,"iteration":iteration,"seed":seed_snapshot,
                        "submission":submission.resource_observation(),"stats":format!("{:?}",world.stats),
                        "changed_from_seed":changed(&seed_bytes,&bytes),"changed_from_first":first.as_ref().map(|v|changed(v,&bytes)),
                        "zero_alpha_pixels":bytes.chunks_exact(4).filter(|p|p[3]==0).count()});
                    std::fs::write(out.join(format!("{cell}-{iteration}.json")),serde_json::to_vec_pretty(&row).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
                    rows.push(row);if first.is_none(){first=Some(bytes);}drop(submission);drop(frame);
                }
                drop(world);drop(adapter);
            }}
        }
        std::fs::write(out.join("report.json"),serde_json::to_vec_pretty(&serde_json::json!({"scope":"Closed-only Intel ownership diagnostic; no production change or acceptance claim","sample_diagnostics":sample_diagnostics,"frames":rows})).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
        Ok(())
    })
}
