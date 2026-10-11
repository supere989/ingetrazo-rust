//! Native production World+annotation candidate acceptance; no source pixels substituted.
use super::*;
use glam::DVec3;
use std::sync::Arc;
pub(crate) fn run(input:PathBuf,output:PathBuf)->Result<(),String>{
 std::fs::create_dir_all(&output).map_err(|e|e.to_string())?;
 if std::fs::read_dir(&output).map_err(|e|e.to_string())?.next().is_some(){return Err("Fresh document annotation output required".into());}
 let metal=Arc::new(NativeMetal::new()?);let ctx=egui::Context::default();
 let mut painter=if ingetrazo_metal::MetalCad2d::supported(&metal){ingetrazo_metal::egui_painter::EguiPainter::new(&metal)?}else{ingetrazo_metal::egui_painter::EguiPainter::new_compute(&metal)?};
 let mut app=IngeTrazoApp::new();app.scene=ingetrazo_formats::igz::load_igz(&input).map_err(|e|e.to_string())?;app.apply_document_presentation();app.zoom_extents();app.camera.aspect=800./600.;
 if app.scene.dimensions.len()!=4||!app.scene.show_section_cuts||app.scene.section_planes.len()!=1||app.scene.section_planes.iter().any(|p|p.active)||app.scene.show_section_fill{return Err("Pinned pergola stage inventory changed".into());}
 app.native_metal_world=Some(world_viewport::Viewport::new(metal.clone()));
 let capture=|app:&mut IngeTrazoApp,painter:&mut ingetrazo_metal::egui_painter::EguiPainter,route|world_sweep_probe::frame(app,&metal,&ctx,painter,route);
 let full=capture(&mut app,&mut painter,true)?;let counters=app.native_metal_world.as_ref().unwrap().label_stats();let geometry=app.native_metal_world.as_ref().unwrap().geometry_uploads();
 for _ in 0..3{assert_eq!(capture(&mut app,&mut painter,true)?,full);assert_eq!(app.native_metal_world.as_ref().unwrap().label_stats(),counters);assert_eq!(app.native_metal_world.as_ref().unwrap().geometry_uploads(),geometry);}
 let dimensions=std::mem::take(&mut app.scene.dimensions);app.bump_model_revision();let off=capture(&mut app,&mut painter,true)?;assert_ne!(full,off,"Four dimensions produced no visible native pixels");
 app.scene.dimensions=dimensions;app.bump_model_revision();assert_eq!(capture(&mut app,&mut painter,true)?,full);
 app.scene.show_section_cuts=false;let uncut=capture(&mut app,&mut painter,true)?;assert_eq!(full,uncut,"Inactive pergola plane changed native pixels");app.scene.show_section_cuts=true;assert_eq!(capture(&mut app,&mut painter,true)?,full);
 // Force a changed candidate through real font admission. The accepted native
 // World+callback remain intact; refusal is not an accepted dimension-less frame.
 let old=app.native_metal_world.as_ref().unwrap().label_owner().unwrap();let weak=Arc::downgrade(&old);drop(old);
 let original_offset=app.scene.dimensions[0].offset;
 app.native_metal_world.as_mut().unwrap().probe_font_budget(true)?;app.scene.dimensions[0].offset.x+=0.01;app.bump_model_revision();let rect=egui::Rect::from_min_size(egui::Pos2::ZERO,egui::vec2(800.,600.));
 let font_error=world_viewport::route(&mut app,rect,1.).ok_or("Font quota unexpectedly admitted")?;assert_eq!(capture(&mut app,&mut painter,false)?,full);assert!(weak.upgrade().is_some());
 app.scene.dimensions[0].offset=original_offset;app.bump_model_revision();app.native_metal_world.as_mut().unwrap().probe_font_budget(false)?;assert_eq!(capture(&mut app,&mut painter,true)?,full);
 // A genuine CAD allocation refusal retains the complete accepted image/charge.
 let charges=metal.budget.lock().map_err(|_|"Budget poisoned")?.heaps.iter().map(|h|h.committed).sum::<u64>();
 let limit={let mut b=metal.budget.lock().map_err(|_|"Budget poisoned")?;let old=b.total_limit;b.total_limit=Some(0);old};
 app.scene.dimensions[0].offset.x+=0.01;app.bump_model_revision();let gpu_error=world_viewport::route(&mut app,rect,1.).ok_or("GPU quota unexpectedly admitted")?;assert_eq!(metal.budget.lock().map_err(|_|"Budget poisoned")?.heaps.iter().map(|h|h.committed).sum::<u64>(),charges);
 metal.budget.lock().map_err(|_|"Budget poisoned")?.total_limit=limit;assert_eq!(capture(&mut app,&mut painter,false)?,full);app.scene.dimensions[0].offset=original_offset;app.bump_model_revision();assert_eq!(capture(&mut app,&mut painter,true)?,full);
 // Same actual production route: a large opaque plane hides every dimension
 // query, while moving that same dimension in front gives a positive image.
 let mut scene=ingetrazo_core::scene::Scene::new();let v=[DVec3::new(-10.,-10.,1.),DVec3::new(10.,-10.,1.),DVec3::new(10.,10.,1.),DVec3::new(-10.,10.,1.)].map(|p|scene.mesh.add_vertex(p));scene.mesh.add_face(v.to_vec(),vec![]).ok_or("Depth control face")?;
 app.scene=scene;app.camera.target=DVec3::ZERO;app.camera.distance=5.;app.camera.yaw=0.;app.camera.pitch=std::f64::consts::FRAC_PI_2;app.camera.perspective=false;app.bump_model_revision();let depth_off=capture(&mut app,&mut painter,true)?;
 app.scene.dimensions.push(ingetrazo_core::dimensions::Dimension::new(DVec3::new(-1.,0.,0.),DVec3::new(1.,0.,0.),DVec3::new(0.,0.2,0.)));app.bump_model_revision();let hidden=capture(&mut app,&mut painter,true)?;assert_eq!(hidden,depth_off,"Occluded dimension leaked native pixels");
 app.scene.dimensions[0].start.z=2.;app.scene.dimensions[0].end.z=2.;app.bump_model_revision();let visible=capture(&mut app,&mut painter,true)?;assert_ne!(visible,depth_off,"In-front dimension did not draw");
 for(name,bytes)in[("pergola",full),("dimensions-off",off),("section-off",uncut),("depth-off",depth_off),("depth-hidden",hidden),("depth-visible",visible)]{std::fs::write(output.join(format!("{name}.rgba")),bytes).map_err(|e|e.to_string())?;}
 drop(painter);drop(app);let b=metal.budget.lock().map_err(|_|"Budget poisoned")?;let committed=b.heaps.iter().map(|h|h.committed).sum::<u64>();let reserved=b.heaps.iter().map(|h|h.reserved).sum::<u64>();assert_eq!((committed,reserved),(0,0));
 std::fs::write(output.join("report.json"),serde_json::to_vec_pretty(&serde_json::json!({"passed":true,"extent":[800,600],"samples":4,"dimensions":4,"active_section":false,"inactive_section_toggle_exact":true,"font_refusal":font_error,"gpu_refusal":gpu_error,"warm_extra_annotation_uploads":0,"warm_extra_world_uploads":0,"depth_hidden_exact":true,"depth_visible_contrast":true,"ledger_committed":committed,"ledger_reserved":reserved})).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;Ok(())
}
