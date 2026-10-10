//! Physical composer geometry for direct native CAD submission.
//! Tessellation uses white only: authored color never crosses Color32.
use std::sync::Arc;
use egui::{Pos2,Color32,Stroke};
use glam::DVec2;
use ingetrazo_core::sheet_shape::{SheetShape,ShapePath};
use ingetrazo_render::cad2d::{Geometry,Vertex,Batch};

fn build_mapped(ctx:&egui::Context,shape:&SheetShape,zoom:f32,pixels_per_point:f32)->Result<(Arc<Geometry>,Arc<[Batch]>,Vec<(usize,bool)>),String>{
 if !zoom.is_finite()||zoom<=0.||!pixels_per_point.is_finite()||pixels_per_point<=0.{return Err("Invalid composer CAD scale".into());}
 let density=zoom*pixels_per_point;if !density.is_finite(){return Err("Composer CAD scale overflow".into());}
 let commands=shape.paint_commands(100_000)?;
 let palette=crate::composer::precise_shape_palette(&commands)?;
 let mut vertices=Vec::new();let mut indices=Vec::new();let mut batches=Vec::new();let mut mapping=Vec::new();
 for (command_index,(command,(stroke_color,fill_color))) in commands.into_iter().zip(palette).enumerate(){
  let (points,closed)=match command.path {
   ShapePath::RoundedRect{size,radius}=>{
    let points=if radius<=0.{vec![DVec2::ZERO,DVec2::new(size.x,0.),size,DVec2::new(0.,size.y)]}else{
     let mut p=Vec::with_capacity(100);
     for(center,start)in[(DVec2::new(size.x-radius,radius),-std::f64::consts::FRAC_PI_2),(size-DVec2::splat(radius),0.),(DVec2::new(radius,size.y-radius),std::f64::consts::FRAC_PI_2),(DVec2::splat(radius),std::f64::consts::PI)]{
      for i in 0..=24{let angle=start+std::f64::consts::FRAC_PI_2*f64::from(i)/24.;p.push(center+DVec2::new(angle.cos(),angle.sin())*radius);}
     }p
    };(points,true)
   },
   ShapePath::Ellipse{size}=>{let c=size/2.;((0..96).map(|i|{let a=std::f64::consts::TAU*f64::from(i)/96.;c+DVec2::new(c.x*a.cos(),c.y*a.sin())}).collect(),true)},
   ShapePath::Polygon(points)=>(points,true),
   ShapePath::Line(mut a,mut b)=>{
    if let Some(clip)=command.clip{
     let delta=b-a;let mut lo=0_f64;let mut hi=1_f64;
     let sign=clip.iter().zip(clip.iter().cycle().skip(1)).map(|(a,b)|a.perp_dot(*b)).sum::<f64>().signum();
     for i in 0..clip.len(){let e=clip[(i+1)%clip.len()]-clip[i];let base=sign*e.perp_dot(a-clip[i]);let slope=sign*e.perp_dot(delta);if slope.abs()<1e-12{if base<0.{lo=1.;hi=0.;break;}}else{let t=-base/slope;if slope>0.{lo=lo.max(t);}else{hi=hi.min(t);}}}
     if lo>hi{continue;}b=a+delta*hi;a+=delta*lo;
    }(vec![a,b],false)
   }
  };
  let points=points.into_iter().map(|p|Pos2::new(p.x as f32,p.y as f32)).collect::<Vec<_>>();
  if points.iter().any(|p|!p.x.is_finite()||!p.y.is_finite()){return Err("Composer CAD coordinate overflow".into());}
  for (color,fill) in [(fill_color,true),(stroke_color,false)]{
   let Some(color)=color else{continue;};
   let width=command.stroke.as_ref().map(|(_,w)|*w as f32).unwrap_or(0.);
   let mut shapes=vec![egui::epaint::ClippedShape{clip_rect:egui::Rect::EVERYTHING,shape:egui::epaint::PathShape{points:points.clone(),closed,fill:if fill{Color32::WHITE}else{Color32::TRANSPARENT},stroke:if fill{Stroke::NONE}else{Stroke::new(width,Color32::WHITE)}.into()}.into()}];
   if !fill&&!closed&&width>0.{for point in &points{shapes.push(egui::epaint::ClippedShape{clip_rect:egui::Rect::EVERYTHING,shape:egui::Shape::circle_filled(*point,width/2.,Color32::WHITE)});}}
   let first=u32::try_from(indices.len()).map_err(|_|"Composer CAD index overflow")?;
   for primitive in ctx.tessellate(shapes,density){
    let egui::epaint::Primitive::Mesh(mesh)=primitive.primitive else{return Err("Unexpected callback in composer CAD tessellation".into());};
    let base=u32::try_from(vertices.len()).map_err(|_|"Composer CAD vertex overflow")?;
    vertices.extend(mesh.vertices.into_iter().map(|v|Vertex{position:[v.pos.x,v.pos.y],coverage:f32::from(v.color.a())/255.}));
    for index in mesh.indices{indices.push(base.checked_add(index).ok_or("Composer CAD index overflow")?);}
   }
   let end=u32::try_from(indices.len()).map_err(|_|"Composer CAD index overflow")?;
   if end>first{batches.push(Batch{first,count:end-first,color});mapping.push((command_index,fill));}
  }
 }
 let geometry=Geometry::new(vertices,indices)?;geometry.validate_batches(&batches)?;Ok((geometry,batches.into(),mapping))
}
pub fn build(ctx:&egui::Context,shape:&SheetShape,zoom:f32,pixels_per_point:f32)->Result<(Arc<Geometry>,Arc<[Batch]>),String>{let(g,b,_)=build_mapped(ctx,shape,zoom,pixels_per_point)?;Ok((g,b))}
#[derive(Default)]pub struct Cache{shape:Option<SheetShape>,density:u32,geometry:Option<Arc<Geometry>>,batches:Option<Arc<[Batch]>>,mapping:Vec<(usize,bool)>}
fn same_geometry(a:&SheetShape,b:&SheetShape)->bool{
 a.kind==b.kind&&a.w_mm==b.w_mm&&a.h_mm==b.h_mm&&a.stroke_mm==b.stroke_mm&&a.fill==b.fill&&a.invert==b.invert&&a.radius_mm==b.radius_mm&&a.sides==b.sides&&a.ground==b.ground&&a.tick_mm==b.tick_mm&&a.tick_step_mm==b.tick_step_mm&&a.band_mm==b.band_mm
}
impl Cache{
 /// Stable pose/palette changes retain immutable geometry identity. Warm calls
 /// clone only two Arcs; no path generation, tessellation or palette allocation.
 pub fn prepare(&mut self,ctx:&egui::Context,shape:&SheetShape,zoom:f32,pixels_per_point:f32)->Result<(Arc<Geometry>,Arc<[Batch]>),String>{
  if !zoom.is_finite()||zoom<=0.||!pixels_per_point.is_finite()||pixels_per_point<=0.||!(zoom*pixels_per_point).is_finite(){return Err("Invalid composer CAD scale".into());}
  let density=(zoom*pixels_per_point).to_bits();
  let geometry_matches=self.shape.as_ref().is_some_and(|s|same_geometry(s,shape))&&self.density==density;
  if !geometry_matches{
   let(g,b,m)=build_mapped(ctx,shape,zoom,pixels_per_point)?;
   self.geometry=Some(g);self.batches=Some(b);self.mapping=m;self.shape=Some(shape.clone());self.density=density;
  }else if self.shape.as_ref().is_some_and(|s|s.color!=shape.color||s.fill_color!=shape.fill_color){
   let commands=shape.paint_commands(100_000)?;let palette=crate::composer::precise_shape_palette(&commands)?;
   let old=self.batches.as_ref().unwrap();let mut batches=Vec::with_capacity(old.len());
   for (old,(command,fill)) in old.iter().zip(&self.mapping){let pair=palette.get(*command).ok_or("Composer CAD palette mapping invalid")?;let color=if *fill{pair.1}else{pair.0}.ok_or("Composer CAD palette role missing")?;batches.push(Batch{color,..*old});}
   self.geometry.as_ref().unwrap().validate_batches(&batches)?;
   self.batches=Some(batches.into());self.shape=Some(shape.clone());
  }
  Ok((self.geometry.as_ref().unwrap().clone(),self.batches.as_ref().unwrap().clone()))
 }
}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn pose_palette_and_invalid_updates_preserve_cached_geometry(){
  let ctx=egui::Context::default();let _=ctx.run(Default::default(),|_|{});let mut cache=Cache::default();let mut shape=SheetShape::default();shape.fill=true;
  let(g,b)=cache.prepare(&ctx,&shape,4.,1.).unwrap();shape.x_mm+=17.;shape.z+=1.;
  let(g2,b2)=cache.prepare(&ctx,&shape,4.,1.).unwrap();assert!(Arc::ptr_eq(&g,&g2)&&Arc::ptr_eq(&b,&b2));
  shape.fill_color="#80255d9d".into();let(g3,b3)=cache.prepare(&ctx,&shape,4.,1.).unwrap();assert!(Arc::ptr_eq(&g,&g3));assert!(!Arc::ptr_eq(&b,&b3));
  shape.fill_color="bad-color".into();assert!(cache.prepare(&ctx,&shape,4.,1.).is_err());assert!(Arc::ptr_eq(cache.geometry.as_ref().unwrap(),&g));assert!(Arc::ptr_eq(cache.batches.as_ref().unwrap(),&b3));
 }
 #[test]fn direct_geometry_separates_coverage_from_authored_alpha(){
  let ctx=egui::Context::default();let _=ctx.run(Default::default(),|_|{});
  let mut shape=SheetShape::default();shape.fill=true;shape.fill_color="#80255d9d".into();shape.stroke_mm=0.;
  let (geometry,batches)=build(&ctx,&shape,4.,1.).unwrap();
  assert!(!geometry.indices().is_empty());assert_eq!(batches[0].color.0,[37./255.,93./255.,157./255.,128./255.]);
  assert!(geometry.vertices().iter().any(|v|v.coverage==1.));
  assert!(build(&ctx,&shape,0.,1.).is_err());
 }
}
