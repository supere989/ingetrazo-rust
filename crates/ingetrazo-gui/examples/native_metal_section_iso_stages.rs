//! ARM closed-iso stage observation with an explicit cold positive-depth candidate.
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

        let vertex_replay=std::env::args().nth(3).as_deref()==Some("--vertex-replay");
        let candidate = match std::env::args().nth(3).as_deref() { None | Some("--vertex-replay") => false, Some("--positive-depth-viewport") => true, _ => return Err("Unknown observation mode".into()) };
        let metal = NativeMetal::new()?;
        let sample_diagnostics = ingetrazo_metal::annotation::sample_diagnostics(&metal);
        let mut rows = vec![];
        for _name in ["closed-iso"] {
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
            camera.set_standard_view(StandardView::Iso);
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


            let expected_path=std::env::args().nth(2).ok_or("Authenticated expected closed-iso RGBA path required")?;
            let expected=std::fs::read(&expected_path).map_err(|e|e.to_string())?;
            if expected.len()!=1_920_000{return Err("Expected whole-frame size mismatch".into());}
            let mut adapter=Adapter::default();let mut world=World::default();world.set_sample_count(&metal,4)?;
            if candidate { world.enable_positive_depth_viewport_probe()?; }
            let frame=prepare(&mut world,&mut adapter,&scene)?;
            if !frame.section_contours_enabled(){return Err("Iso contours absent".into());}
            let baseline=frame.submit_diagnostic(&metal)?;
            let ordinary=baseline.readback(std::time::Duration::from_secs(5))?;
            std::fs::write(out.join("closed-iso-ordinary.rgba"),&ordinary).map_err(|e|e.to_string())?;
            if ordinary.len()!=1_920_000{return Err("Ordinary whole-frame extent mismatch".into());}
            let domain=frame.positive_depth_domain_receipt()?;
            if candidate && world.enable_positive_depth_viewport_probe().is_ok(){return Err("Warm positive-depth opt-in unexpectedly admitted".into());}
            if !candidate && ordinary!=expected{return Err("Ordinary closed-iso frame differs from authenticated retained134-residual frame".into());}
            drop(baseline);
            if vertex_replay {let replay=frame.observe_compiled_vertex_replay(&metal)?;std::fs::write(out.join("vertex-replay.json"),serde_json::to_vec_pretty(&replay).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;}
            let points=[[297,329],[297,328],[298,303]];
            let diagnostic=frame.submit_iso_stages(&metal,points,None)?;
            let bytes=diagnostic.readback(std::time::Duration::from_secs(5))?;
            std::fs::write(out.join("closed-iso-observed.rgba"),&bytes).map_err(|e|e.to_string())?;
            if bytes!=ordinary{return Err("Iso observation perturbed whole frame".into());}
            let full_colors=diagnostic.iso_stage_colors()?;
            let mut prefixes=vec![];
            for stage in 0..4u8 {
                let prefix=frame.submit_iso_stages(&metal,points,Some(stage))?;
                let rgba=prefix.readback(std::time::Duration::from_secs(5))?;
                if rgba.len()!=1_920_000{return Err("Iso prefix byte extent mismatch".into());}
                std::fs::write(out.join(format!("prefix-{stage}.rgba")),rgba).map_err(|e|e.to_string())?;
                let(c,d)=prefix.iso_values()?;
                if c.map(|v|v.map(f32::to_bits))!=full_colors[stage as usize].map(|v|v.map(f32::to_bits)){return Err(format!("Prefix{stage} color differs from uninterrupted in-pass checkpoint"));}
                prefixes.push(serde_json::json!({"stage":stage,"colors":c,"color_bits":c.map(|v|v.map(f32::to_bits)),"depth":d,"depth_bits":d.map(f32::to_bits),"resource":prefix.resource_observation(),"color_matches_in_pass":true}));
            }
            let (colors,depths)=diagnostic.iso_values()?;
            for(i,p)in points.iter().enumerate(){let offset=(p[1]as usize*800+p[0]as usize)*4;
                rows.push(serde_json::json!({"point_top_origin":p,"resolved_rgba8":&bytes[offset..offset+4],
                    "sample_rgba_float":&colors[i*4..i*4+4],"sample_rgba_bits":colors[i*4..i*4+4].iter().map(|v|v.map(f32::to_bits)).collect::<Vec<_>>(),
                    "sample_depth_float":&depths[i*4..i*4+4],"sample_depth_bits":depths[i*4..i*4+4].iter().map(|v|v.to_bits()).collect::<Vec<_>>() }));}
            std::fs::write(out.join("report.json"),serde_json::to_vec_pretty(&serde_json::json!({"case":"closed-iso","pixels":rows,"prefixes":prefixes,"stage_colors":full_colors,"stage_color_bits":full_colors.map(|stage|stage.map(|v|v.map(f32::to_bits))),
                "resource":diagnostic.resource_observation(),"sample_diagnostics":sample_diagnostics,"material_commands":frame.material_diagnostic()?,
                "candidate_positive_depth_viewport":candidate,"positive_depth_domain":domain,"warm_opt_in_refused":candidate,"ordinary_and_observed_equal":true,"ordinary_and_observed_equal_authenticated_retained":!candidate,"scope":"Read-only in-pass stage colors plus independently terminated prefix depth, attributed only after exact12color equality. No split/resumed render pass or production change."})).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
            drop(diagnostic);drop(frame);drop(world);drop(adapter);
        }
        Ok(())
    })
}
