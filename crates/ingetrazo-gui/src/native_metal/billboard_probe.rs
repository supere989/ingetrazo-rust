//! Billboard-only production route checks; the separate document consumer retains dimensions.
use super::*;
use std::sync::Arc;
pub(crate) fn run(input:PathBuf,output:PathBuf)->Result<(),String>{
 std::fs::create_dir_all(&output).map_err(|e|e.to_string())?;
 if std::fs::read_dir(&output).map_err(|e|e.to_string())?.next().is_some(){return Err("Fresh billboard output required".into());}
 let metal=Arc::new(NativeMetal::new()?);let ctx=egui::Context::default();
 let mut painter=if ingetrazo_metal::MetalCad2d::supported(&metal){ingetrazo_metal::egui_painter::EguiPainter::new(&metal)?}else{ingetrazo_metal::egui_painter::EguiPainter::new_compute(&metal)?};
 let mut app=IngeTrazoApp::new();app.scene=ingetrazo_formats::igz::load_igz(&input).map_err(|e|e.to_string())?;app.apply_document_presentation();app.zoom_extents();app.camera.aspect=800./600.;
 let dimension_count=app.scene.dimensions.len();app.scene.dimensions.clear();
 app.native_metal_world=Some(world_viewport::Viewport::new(metal.clone()));
 let capture=|app:&mut IngeTrazoApp,painter:&mut ingetrazo_metal::egui_painter::EguiPainter,route|world_sweep_probe::frame(app,&metal,&ctx,painter,route);
 let full=capture(&mut app,&mut painter,true)?;let stats=app.native_metal_world.as_ref().unwrap().sweep_stats();assert!(stats.billboard_uploads>0,"Original fixture has no visible billboard owner");
 for _ in 0..3{assert_eq!(capture(&mut app,&mut painter,true)?,full);let now=app.native_metal_world.as_ref().unwrap().sweep_stats();assert_eq!((now.geometry_uploads,now.billboard_uploads,now.billboard_texture_uploads),(stats.geometry_uploads,stats.billboard_uploads,stats.billboard_texture_uploads));}
 let yaw=app.camera.yaw;app.camera.yaw+=0.25;let turned=capture(&mut app,&mut painter,true)?;assert_ne!(full,turned);let turned_stats=app.native_metal_world.as_ref().unwrap().sweep_stats();assert_eq!(turned_stats.geometry_uploads,stats.geometry_uploads);assert!(turned_stats.billboard_uploads>stats.billboard_uploads);assert_eq!(turned_stats.billboard_texture_uploads,stats.billboard_texture_uploads);app.camera.yaw=yaw;assert_eq!(capture(&mut app,&mut painter,true)?,full);
 let groups=app.scene.groups.clone();fn hide(gs:&mut[ingetrazo_core::groups::Group]){for g in gs{if g.billboard{g.hidden=true;}hide(&mut g.children);}}
 hide(&mut app.scene.groups);app.bump_model_revision();let hidden=capture(&mut app,&mut painter,true)?;assert_ne!(hidden,full,"Visible billboard produced no pixels");app.scene.groups=groups;app.bump_model_revision();assert_eq!(capture(&mut app,&mut painter,true)?,full);
 // Camera-only allocation refusal must preserve the accepted frame and ledger.
 let before=metal.budget.lock().map_err(|_|"Budget poisoned")?.heaps.iter().map(|h|h.committed).sum::<u64>();let limit={let mut b=metal.budget.lock().map_err(|_|"Budget poisoned")?;let old=b.total_limit;b.total_limit=Some(0);old};app.camera.yaw+=0.1;
 let rect=egui::Rect::from_min_size(egui::Pos2::ZERO,egui::vec2(800.,600.));let refusal=world_viewport::route(&mut app,rect,1.).ok_or("Billboard quota unexpectedly admitted")?;
 assert_eq!(metal.budget.lock().map_err(|_|"Budget poisoned")?.heaps.iter().map(|h|h.committed).sum::<u64>(),before);assert_eq!(capture(&mut app,&mut painter,false)?,full);
 metal.budget.lock().map_err(|_|"Budget poisoned")?.total_limit=limit;app.camera.yaw=yaw;assert_eq!(capture(&mut app,&mut painter,true)?,full);
 for(name,bytes)in[("baseline",full),("turned",turned),("hidden",hidden)]{std::fs::write(output.join(format!("{name}.rgba")),bytes).map_err(|e|e.to_string())?;}
 // A single cold image used by ordinary and billboard geometry has one native owner.
 let mut scene=ingetrazo_core::scene::Scene::new();let mut encoded=std::io::Cursor::new(Vec::new());image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(2,2,image::Rgba([16,230,92,255]))).write_to(&mut encoded,image::ImageFormat::Png).map_err(|e|e.to_string())?;
 scene.embedded_textures.insert("shared-billboard-control.png".into(),encoded.into_inner());let texture=serde_json::json!({"embed":"shared-billboard-control.png","sw":1.,"sh":2.});
 fn quad(x:f64)->ingetrazo_core::mesh::Mesh{let mut mesh=ingetrazo_core::mesh::Mesh::new();let ids=[[x,0.,0.],[x+1.,0.,0.],[x+1.,0.,2.],[x,0.,2.]].map(|v|mesh.add_vertex(glam::DVec3::from_array(v)));mesh.add_face(ids.to_vec(),vec![]).unwrap();mesh}
 scene.mesh=quad(-2.);scene.mesh.faces[0].attrs.insert("texture".into(),texture.clone());let mut group=ingetrazo_core::groups::Group::with_mesh("Shared image billboard",quad(1.));group.billboard=true;group.mesh.faces[0].attrs.insert("texture".into(),texture);scene.groups.push(group);
 app.scene=scene;app.camera.target=glam::DVec3::new(0.,0.,1.);app.camera.distance=8.;app.camera.yaw=-std::f64::consts::FRAC_PI_2;app.camera.pitch=0.;app.camera.perspective=false;app.bump_model_revision();let image_before=app.native_metal_world.as_ref().unwrap().sweep_stats();let _=capture(&mut app,&mut painter,true)?;let image_after=app.native_metal_world.as_ref().unwrap().sweep_stats();let shared=app.native_metal_world.as_ref().unwrap().shared_billboard_images();assert_eq!(shared,1);assert_eq!((image_after.texture_uploads+image_after.billboard_texture_uploads)-(image_before.texture_uploads+image_before.billboard_texture_uploads),1,"Shared image was uploaded twice");
 drop(painter);drop(app);let b=metal.budget.lock().map_err(|_|"Budget poisoned")?;let committed=b.heaps.iter().map(|h|h.committed).sum::<u64>();let reserved=b.heaps.iter().map(|h|h.reserved).sum::<u64>();assert_eq!((committed,reserved),(0,0));
 std::fs::write(output.join("report.json"),serde_json::to_vec_pretty(&serde_json::json!({"passed":true,"extent":[800,600],"dimensions_omitted_for_stage_probe":dimension_count,"billboard_uploads":stats.billboard_uploads,"warm_extra_billboard_uploads":0,"camera_extra_ordinary_uploads":0,"camera_extra_image_uploads":0,"hide_contrast_restore_exact":true,"quota_refusal":refusal,"shared_ordinary_billboard_image_owners":shared,"shared_image_uploads":1,"ledger_committed":committed,"ledger_reserved":reserved})).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;Ok(())
}
