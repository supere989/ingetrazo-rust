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
        let reflected_mode=std::env::args().any(|a|a=="--reflected-diagnostic");
        let production_mode=std::env::args().any(|a|a=="--production");
        let metal = NativeMetal::new()?;
        let sample_diagnostics = ingetrazo_metal::annotation::sample_diagnostics(&metal);
        if reflected_mode {
            ingetrazo_metal::annotation::reflected_sample_diagnostic_policy(&metal)?;
            if ingetrazo_metal::annotation::source_sample_policy(&metal,4).is_ok(){return Err("Diagnostic must not waive eligible production policy".into());}
        }
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
            world.use_native_resolve_reference()?;
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
            let reference=bytes.clone();
            let reference_diagnostics=frame.programmed_sample_positions();
            drop(frame);
            drop(world);
            let mut world=World::default();
            let mut adapter=Adapter::default();
            if !production_mode && !reflected_mode {world.use_native_resolve_reference()?;}
            world.set_sample_count(&metal,4)?;
            if reflected_mode {
                world.enable_reflected_sample_diagnostic(&metal)?;world.enable_sample_observation()?;
                if world.set_sample_count(&metal,1).is_ok(){return Err("Reflected mode admitted incompatible sample count".into());}
            }
            else if !production_mode {world.enable_source_resolve_prototype(&metal)?;}
            let frame=prepare(&mut world,&mut adapter,&scene)?;
            let resolve_diagnostics=frame.source_resolve_diagnostic();
            let sample_observation=if reflected_mode {
                let points=[[400,228],[328,300],[471,300],[400,371]];
                let diagnostic=frame.submit_sample_diagnostic(&metal,points)?;
                let pixels=diagnostic.readback(std::time::Duration::from_secs(5))?;
                let samples=diagnostic.sample_values()?;let depths=diagnostic.sample_depth_values()?;
                std::fs::write(out.join(format!("{name}-samples.json")),serde_json::to_vec_pretty(&serde_json::json!({"points_top_origin":points,"sample_rgba_bits":samples.map(|p|p.map(f32::to_bits)),"sample_depth_bits":depths.map(f32::to_bits),"selected_source_sample":0,"transport":diagnostic.resource_observation()})).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
                Some(pixels)
            }else{None};
            let bytes=if let Some(pixels)=sample_observation {pixels}else{frame.submit_diagnostic(&metal)?.readback(std::time::Duration::from_secs(5))?};
            std::fs::write(out.join(format!("{name}-candidate.rgba")),&bytes).map_err(|e|e.to_string())?;
            let changed_pixels=reference.chunks_exact(4).zip(bytes.chunks_exact(4)).filter(|(a,b)|a!=b).count();
            drop(frame);
            if reflected_mode && world.enable_reflected_sample_diagnostic(&metal).is_ok(){return Err("Warm reflected mode replacement admitted".into());}
            let before = world.stats;
            let mut repeated = None;
            for iteration in 0..3 {
                let frame = prepare(&mut world, &mut adapter, &scene)?;
                let pixels = frame
                    .submit_diagnostic(&metal)?
                    .readback(std::time::Duration::from_secs(5))?;
                if pixels != bytes {
                    // Failure-only evidence; preserve exact bytes before refusal.
                    std::fs::write(out.join(format!("{name}-warm-{iteration}-expected.rgba")),&bytes).map_err(|e|e.to_string())?;
                    std::fs::write(out.join(format!("{name}-warm-{iteration}-actual.rgba")),&pixels).map_err(|e|e.to_string())?;
                    let evidence=serde_json::json!({"case":name,"iteration":iteration,"production_mode":production_mode,"extent":frame.extent(),"samples":frame.sample_count(),"first_route":&resolve_diagnostics,"warm_route":frame.source_resolve_diagnostic(),"first_programmed_sample_positions":programmed_positions,"warm_programmed_sample_positions":frame.programmed_sample_positions(),"before_counters":format!("{:?}",before),"warm_counters":format!("{:?}",world.stats),"changed_pixels":bytes.chunks_exact(4).zip(pixels.chunks_exact(4)).filter(|(a,b)|a!=b).count()});
                    std::fs::write(out.join(format!("{name}-warm-{iteration}-failure.json")),serde_json::to_vec_pretty(&evidence).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
                    return Err(format!("Warm pixels changed: {name}"));
                }
                repeated = Some(pixels);
                drop(frame);
            }
            let after = world.stats;
            if before.target_allocations != after.target_allocations
                || before.source_resolve_pipeline_builds!=after.source_resolve_pipeline_builds
                || before.section_contour_pipeline_builds!=after.section_contour_pipeline_builds
                || before.geometry_uploads != after.geometry_uploads || before.section_fill_uploads!=after.section_fill_uploads || before.section_fill_upload_bytes!=after.section_fill_upload_bytes
            {
                return Err(format!("Warm allocation changed: {name}"));
            }
            let mut diagnostic_controls=serde_json::Value::Null;
            if reflected_mode && name=="closed" {
                let held=prepare(&mut world,&mut adapter,&scene)?;
                let ticket={let mut ledger=metal.budget.lock().map_err(|_|"Diagnostic budget lock")?;
                    let heap=&ledger.heaps[0];let available=heap.limit-heap.committed-heap.reserved;
                    ledger.reserve(0,available)?};
                let refusal=prepare(&mut world,&mut adapter,&scene).err();
                metal.budget.lock().map_err(|_|"Diagnostic budget lock")?.cancel(ticket)?;
                if refusal.is_none(){return Err("Pinned-target budget refusal did not occur".into());}
                let retained=held.submit_diagnostic(&metal)?.readback(std::time::Duration::from_secs(5))?;
                if retained!=bytes{return Err("Held diagnostic frame changed after refused target allocation".into());}
                let foreign=NativeMetal::new()?;
                if world.set_sample_count(&foreign,4).is_ok(){return Err("Foreign diagnostic context admitted".into());}
                if world.enable_source_resolve_prototype(&metal).is_ok(){return Err("Warm resolve mode replacement admitted".into());}
                diagnostic_controls=serde_json::json!({"budget_refusal":refusal,"held_pixels_preserved":true,"foreign_owner_rejected":true,"warm_mode_rejected":true});
            }
            let mut production_toggle=serde_json::Value::Null;
            if name=="closed" && !reflected_mode{
                let mut production=World::default();let mut production_adapter=Adapter::default();
                production.set_sample_count(&metal,4)?;
                let held=prepare(&mut production,&mut production_adapter,&scene)?;
                let first=held.submit_diagnostic(&metal)?.readback(std::time::Duration::from_secs(5))?;
                let route4=held.source_resolve_diagnostic();let cold=production.stats.source_resolve_pipeline_builds;
                production.set_sample_count(&metal,1)?;
                let single=prepare(&mut production,&mut production_adapter,&scene)?;
                if single.source_resolve_diagnostic()["path"]!="untouched-native-reference"{return Err("Single-sample production retained resolve pipeline".into());}
                let _=single.submit_diagnostic(&metal)?.readback(std::time::Duration::from_secs(5))?;drop(single);
                production.set_sample_count(&metal,4)?;
                let restored=prepare(&mut production,&mut production_adapter,&scene)?;
                let restored_pixels=restored.submit_diagnostic(&metal)?.readback(std::time::Duration::from_secs(5))?;
                if restored_pixels!=first||production.stats.source_resolve_pipeline_builds!=cold{return Err("Production4→1→4 changed pixels or rebuilt tile".into());}
                let held_pixels=held.submit_diagnostic(&metal)?.readback(std::time::Duration::from_secs(5))?;
                if held_pixels!=first{return Err("Detached four-sample Prepared ownership changed".into());}
                let eligible=route4["path"]=="same-pass-tile";
                if eligible && first!=bytes{return Err("Production tile differs from diagnostic candidate".into());}
                if !eligible && first!=reference{return Err("Ineligible production changed native reference".into());}
                production_toggle=serde_json::json!({"samples":[4,1,4],"cached_pipeline_builds":cold,"retained_old_frame_equal":true,"restored_equal":true,"single_sample_has_resolve":false,"route":route4});
                drop(restored);drop(held);drop(production_adapter);drop(production);
            }
            scene.show_section_fill = false;
            let off = prepare(&mut world, &mut adapter, &scene)?;
            if !off.section_contours_enabled(){return Err(format!("Fill-off disabled contours: {name}"));}
            let disabled = off
                .submit_diagnostic(&metal)?
                .readback(std::time::Duration::from_secs(5))?;
            std::fs::write(out.join(format!("{name}-candidate-disabled.rgba")), disabled)
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
            rows.push(serde_json::json!({"case":name,"diagnostic_controls":diagnostic_controls,"production_toggle":production_toggle,"resolve_pipeline_builds_before":before.source_resolve_pipeline_builds,"resolve_pipeline_builds_after":after.source_resolve_pipeline_builds,"resolve_prototype":resolve_diagnostics,"candidate_changed_pixels_from_reference":changed_pixels,"reference_sample_positions":reference_diagnostics,"programmed_sample_positions_top_origin":programmed_positions,"width":800,"height":600,"warm_frames":3,"contours_enabled":true,"fill_off_contours_enabled":true,"cuts_off_contours_enabled":false,"section_contour_pipeline_builds_before":before.section_contour_pipeline_builds,"section_contour_pipeline_builds_after":after.section_contour_pipeline_builds,"fill_off_contour_pipeline_builds":fill_off.section_contour_pipeline_builds,"cuts_off_contour_pipeline_builds":cuts_off_stats.section_contour_pipeline_builds,"target_allocations_before":before.target_allocations,"target_allocations_after":after.target_allocations,"geometry_uploads_before":before.geometry_uploads,"geometry_uploads_after":after.geometry_uploads,"section_fill_uploads_before":before.section_fill_uploads,"section_fill_uploads_after":after.section_fill_uploads,"section_fill_upload_bytes_before":before.section_fill_upload_bytes,"section_fill_upload_bytes_after":after.section_fill_upload_bytes}));
            drop(adapter);
            drop(world);
        }
        std::fs::write(out.join("report.json"),serde_json::to_vec_pretty(&serde_json::json!({"production_mode":production_mode,"reflected_diagnostic":reflected_mode,"cases":rows,"sample_diagnostics":sample_diagnostics,"scope":"Diagnostic source-ordered resolve prototype against untouched native reference; ten retained recipes, warm reuse, no release acceptance"})).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
        Ok(())
    })
}
