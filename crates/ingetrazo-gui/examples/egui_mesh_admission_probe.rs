//! CPU-only attribution of actual shared application tessellation; no GPU claim.
fn main()->Result<(),Box<dyn std::error::Error>>{
 let path=std::env::args().nth(1).ok_or("IGZ path required")?;
 let mut rows=Vec::new();
 for size in [[1280.,800.],[1024.,656.],[640.,400.]]{
  let ctx=egui::Context::default();let mut app=ingetrazo_gui::IngeTrazoApp::new();app.open_document(&std::path::PathBuf::from(&path));
  let ids=[glam::DVec3::new(123.,234.,345.),glam::DVec3::new(124.,234.,345.),glam::DVec3::new(123.,235.,345.)].map(|p|app.scene.mesh.add_vertex(p));app.scene.mesh.add_face(ids.to_vec(),vec![]).ok_or("dirty face authoring failed")?;
  for frame in 0..8{
   let input=egui::RawInput{screen_rect:Some(egui::Rect::from_min_max(egui::Pos2::ZERO,egui::pos2(size[0],size[1]))),time:Some(frame as f64*0.016),..Default::default()};
   let output=ctx.run(input,|ctx|app.update_ui(ctx));let primitives=ctx.tessellate(output.shapes,output.pixels_per_point);
   for (primitive,p) in primitives.iter().enumerate(){if let egui::epaint::Primitive::Mesh(mesh)=&p.primitive{
    for (index,v) in mesh.vertices.iter().enumerate(){if !v.pos.is_finite()||!v.uv.is_finite()||v.pos.x.abs()>1e6||v.pos.y.abs()>1e6{
     rows.push(serde_json::json!({"size":size,"frame":frame,"primitive":primitive,"index":index,"pos":[v.pos.x,v.pos.y],"uv":[v.uv.x,v.uv.y],"clip":[p.clip_rect.min.x,p.clip_rect.min.y,p.clip_rect.max.x,p.clip_rect.max.y],"finite_pos":v.pos.is_finite(),"finite_uv":v.uv.is_finite(),"indices_valid":mesh.is_valid()}));break;
    }}
   }}
  }
 }
 println!("{}",serde_json::to_string_pretty(&serde_json::json!({"invalid_meshes":rows,"gpu_claim":false}))?);Ok(())
}
