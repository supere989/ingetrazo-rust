//! Physical composer geometry for direct native CAD submission.
//! Tessellation uses white only: authored color never crosses Color32.
use std::sync::Arc;
use egui::{Pos2,Color32,Stroke};
use glam::DVec2;
use ingetrazo_core::sheet_shape::{SheetShape,ShapePath};
use ingetrazo_render::cad2d::{Geometry,Vertex,Batch};

fn minimum_edge(points:&[DVec2])->f64{points.iter().zip(points.iter().cycle().skip(1)).map(|(a,b)|(*b-*a).length()).fold(f64::INFINITY,f64::min)}
fn convex_outline(points:&[DVec2])->bool{
 if points.len()<3||minimum_edge(points)<=1e-12{return false;}
 let area=points.iter().zip(points.iter().cycle().skip(1)).map(|(a,b)|a.perp_dot(*b)).sum::<f64>();
 if !area.is_finite()||area.abs()<=1e-12{return false;}
 (0..points.len()).all(|i|(points[(i+1)%points.len()]-points[i]).perp_dot(points[(i+2)%points.len()]-points[(i+1)%points.len()])*area>=0.)
}
pub fn convex_bevel_outline(points:&[DVec2],radius:f64)->Result<Vec<Vec<DVec2>>,String>{
 let area=points.iter().zip(points.iter().cycle().skip(1)).map(|(a,b)|a.perp_dot(*b)).sum::<f64>();
 if !convex_outline(points)||!radius.is_finite()||radius<=0.{return Err("Invalid convex bevel stroke".into());}
 let reversed;let points=if area<0.{reversed=points.iter().rev().copied().collect::<Vec<_>>();&reversed[..]}else{points};
 let normals=points.iter().zip(points.iter().cycle().skip(1)).map(|(a,b)|{let d=(*b-*a).normalize();DVec2::new(d.y,-d.x)*radius}).collect::<Vec<_>>();
 let mut outer=Vec::with_capacity(points.len()*2);let mut inner=Vec::with_capacity(points.len()*3);
 for i in 0..points.len(){outer.push(points[i]+normals[i]);outer.push(points[(i+1)%points.len()]+normals[i]);}
 // Literal Qt inside-turn outline retains the original join vertex. Its
 // self-overlapping winding area must not be replaced by a healed polygon.
 for i in (0..points.len()).rev(){inner.push(points[(i+1)%points.len()]-normals[i]);inner.push(points[i]-normals[i]);inner.push(points[i]);}
 Ok(vec![outer,inner])
}

fn build_mapped(ctx:&egui::Context,shape:&SheetShape,zoom:f32,pixels_per_point:f32,phase:DVec2)->Result<(Arc<Geometry>,Arc<[Batch]>,Vec<(usize,bool)>),String>{
 if !zoom.is_finite()||zoom<=0.||!pixels_per_point.is_finite()||pixels_per_point<=0.{return Err("Invalid composer CAD scale".into());}
 let density=zoom*pixels_per_point;if !density.is_finite(){return Err("Composer CAD scale overflow".into());}
 let commands=shape.paint_commands(100_000)?;
 let palette=crate::composer::precise_shape_palette(&commands)?;
 let mut vertices=Vec::new();let mut indices=Vec::new();let mut batches=Vec::new();let mut mapping=Vec::new();let mut ordered_draws=Vec::new();let mut all_span_paths=true;
 for (command_index,(command,(stroke_color,fill_color))) in commands.into_iter().zip(palette).enumerate(){
  let closed_curve=source_closed_curve(&command.path);
  let straight_outline=match &command.path {
   ShapePath::RoundedRect{size,radius} if *radius==0.=>Some(vec![DVec2::ZERO,DVec2::new(size.x,0.),*size,DVec2::new(0.,size.y)]),
   ShapePath::Polygon(points) if convex_outline(points)=>Some(points.clone()),
   _=>None,
  };
  let original_line=if let ShapePath::Line(a,b)=&command.path{Some((*a,*b))}else{None};
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
   ShapePath::Line(a,b)=>{
    (vec![a,b],false)
   }
  };
  let exact_line=original_line;
  let points=points.into_iter().map(|p|Pos2::new(p.x as f32,p.y as f32)).collect::<Vec<_>>();
  if points.iter().any(|p|!p.x.is_finite()||!p.y.is_finite()){return Err("Composer CAD coordinate overflow".into());}
  for (color,fill) in [(fill_color,true),(stroke_color,false)]{
   let Some(color)=color else{continue;};
   let width=command.stroke.as_ref().map(|(_,w)|*w as f32).unwrap_or(0.);
   // Exact convex outlines and source round caps. Other curves, clipped
   // strokes/nonconvex and cosmetic strokes remain on the prior tessellator.
   let eligible_outline=straight_outline.as_ref().filter(|points|fill||(width>0.&&f64::from(width)<minimum_edge(points)));
   let thin=!fill&&command.stroke.as_ref().is_some_and(|(_,w)|*w*f64::from(density)<=1.);
   if thin||eligible_outline.is_some()||(closed_curve.is_some()&&(fill||width>0.))||(!fill&&width>0.&&exact_line.is_some()) {
    let radius=command.stroke.as_ref().map(|(_,w)|*w).unwrap_or(0.)/2.;
    let spans=if thin{
     use ingetrazo_render::qt_gray::{PathContour,Segment};let mut path=if let Some(path)=&closed_curve{path.clone()}else if let Some((a,b))=exact_line{PathContour{start:[a.x,a.y],segments:vec![Segment::Line([b.x,b.y])]}}else if let Some(points)=&straight_outline{let mut segments=points.iter().skip(1).map(|p|Segment::Line([p.x,p.y])).collect::<Vec<_>>();segments.push(Segment::Line([points[0].x,points[0].y]));PathContour{start:[points[0].x,points[0].y],segments}}else{return Err("Cosmetic unsupported authored path".into());};
     let transform=|p:[f64;2]|[p[0]*f64::from(density)+phase.x,p[1]*f64::from(density)+phase.y];path.start=transform(path.start);for segment in &mut path.segments{match segment{Segment::Line(to)=>*to=transform(*to),Segment::Cubic{control1,control2,to}=>{*control1=transform(*control1);*control2=transform(*control2);*to=transform(*to);}}}
     let spans=ingetrazo_render::qt_cosmetic::rasterize_path(&path,closed,command.stroke.as_ref().unwrap().1*f64::from(density),100_000)?;
     if let Some(clip)=&command.clip{let contour=clip.iter().map(|p|transform([p.x,p.y])).collect::<Vec<_>>();let clip=ingetrazo_render::qt_gray::rasterize(&[contour],250_000,100_000)?;
      // Cosmetic segments may overlap and are deliberately not globally sorted.
      let mut clipped=Vec::new();for span in &spans{clipped.extend(ingetrazo_render::qt_gray::intersect_spans(&[*span],&clip,100_000)?);if clipped.len()>100_000{return Err("Cosmetic clipped span admission exceeded".into());}}clipped
     }else{spans}
    }else if let Some(points)=eligible_outline{
    let outlines=if fill{vec![points.clone()]}else{convex_bevel_outline(points,radius)?};
    let mut contours=outlines.into_iter().map(|outline|outline.into_iter().map(|p|[p.x*f64::from(density),p.y*f64::from(density)]).collect::<Vec<_>>()).collect::<Vec<_>>();
    for contour in &mut contours{for point in contour{point[0]+=phase.x;point[1]+=phase.y;}}
    ingetrazo_render::qt_gray::rasterize(&contours,250_000,100_000)?
    }else if let Some(path)=&closed_curve{
     let paths=if fill{vec![path.clone()]}else{source_curve_stroke(path,radius)?};ingetrazo_render::qt_gray::rasterize_transformed_paths(&paths,f64::from(density),[phase.x,phase.y],250_000,100_000)?
    }else{
     let(a,b)=exact_line.unwrap();let Some(path)=round_cap_outline(a,b,radius)else{continue;};
     let spans=ingetrazo_render::qt_gray::rasterize_transformed_paths(&[path],f64::from(density),[phase.x,phase.y],250_000,100_000)?;
     if let Some(clip)=&command.clip{let contour=clip.iter().map(|p|[p.x*f64::from(density)+phase.x,p.y*f64::from(density)+phase.y]).collect::<Vec<_>>();let clip=ingetrazo_render::qt_gray::rasterize(&[contour],250_000,100_000)?;ingetrazo_render::qt_gray::intersect_spans(&spans,&clip,100_000)?}else{spans}
    };
    let byte_count=vertices.len().checked_mul(12).and_then(|v|indices.len().checked_mul(4).and_then(|i|v.checked_add(i))).and_then(|old|spans.len().checked_mul(72).and_then(|new|old.checked_add(new))).ok_or("Composer span byte overflow")?;
    if byte_count>8*1024*1024{return Err("Composer exact span geometry exceeds8MiB admission".into());}
    let first=u32::try_from(indices.len()).map_err(|_|"Composer CAD index overflow")?;
    let ranges=ingetrazo_render::qt_cosmetic::disjoint_ranges(&spans,100_000)?;
    for range in ranges{ordered_draws.push(first+range.start as u32*6..first+range.end as u32*6);}
    for span in spans {
     let base=u32::try_from(vertices.len()).map_err(|_|"Composer CAD vertex overflow")?;
     for position in [[span.x as f64,span.y as f64],[span.x as f64+span.len as f64,span.y as f64],[span.x as f64+span.len as f64,span.y as f64+1.],[span.x as f64,span.y as f64+1.]] {
      vertices.push(Vertex{position:[((position[0]-phase.x)/f64::from(density)) as f32,((position[1]-phase.y)/f64::from(density)) as f32],coverage:f32::from(span.coverage)/255.});
     }
     indices.extend([base,base+1,base+2,base,base+2,base+3]);
    }
    let end=u32::try_from(indices.len()).map_err(|_|"Composer CAD index overflow")?;
    if end>first{batches.push(Batch{first,count:end-first,color});mapping.push((command_index,fill));}
    continue;
   }

   all_span_paths=false;
   let mut shapes=vec![egui::epaint::ClippedShape{clip_rect:egui::Rect::EVERYTHING,shape:egui::epaint::PathShape{points:points.clone(),closed,fill:if fill{Color32::WHITE}else{Color32::TRANSPARENT},stroke:if fill{Stroke::NONE}else{Stroke::new(width,Color32::WHITE)}.into()}.into()}];
   // A round-capped stroke is one covered region. Separate endpoint circles
   // would blend their overlap with the segment twice for translucent paint.
   if !fill&&!closed&&width>0.&&points.len()==2{
    let delta=points[1]-points[0];let angle=delta.y.atan2(delta.x);let radius=width/2.;
    let mut outline=Vec::with_capacity(66);
    for(center,start)in[(points[1],angle-std::f32::consts::FRAC_PI_2),(points[0],angle+std::f32::consts::FRAC_PI_2)]{
     for i in 0..=32{let a=start+std::f32::consts::PI*i as f32/32.;outline.push(center+egui::vec2(a.cos(),a.sin())*radius);}
    }
    shapes=vec![egui::epaint::ClippedShape{clip_rect:egui::Rect::EVERYTHING,shape:egui::epaint::PathShape{points:outline,closed:true,fill:Color32::WHITE,stroke:Stroke::NONE.into()}.into()}];
   }
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
 let geometry=if all_span_paths{Geometry::new_with_ordered_draws(vertices,indices,ordered_draws)?}else{Geometry::new(vertices,indices)?};geometry.validate_batches(&batches)?;Ok((geometry,batches.into(),mapping))
}
pub type Cubic=[DVec2;4];
fn cubic_split(q:Cubic)->(Cubic,Cubic){let ab=(q[0]+q[1])*0.5;let bc=(q[1]+q[2])*0.5;let cd=(q[2]+q[3])*0.5;let abc=(ab+bc)*0.5;let bcd=(bc+cd)*0.5;let m=(abc+bcd)*0.5;([q[0],ab,abc,m],[m,bcd,cd,q[3]])}
fn cubic_point(q:Cubic,t:f64)->DVec2{let mt=1.-t;let a=q[0]*mt+q[1]*t;let b=q[1]*mt+q[2]*t;let c=q[2]*mt+q[3]*t;let a=a*mt+b*t;let b=b*mt+c*t;a*mt+b*t}
// Source QBezier::shift/good_offset, GPL-3.0-only, Qt Company 2016.
// Convex quarter arcs only; degenerate/tiny/reversing offsets refuse admission.
fn shifted_cubic(q:Cubic,offset:f64)->Result<Vec<Cubic>,String>{
 let mut stack=vec![(q,0usize)];let mut out=Vec::new();
 while let Some((q,depth))=stack.pop(){
  let mut normals=[DVec2::ZERO;3];for i in 0..3{let d=q[i+1]-q[i];let length=d.length();if length<=1e-12{return Err("Degenerate source curve offset".into());}normals[i]=DVec2::new(d.y,-d.x)/length;}
  let mut shifted=q;shifted[0]+=normals[0]*offset;shifted[3]+=normals[2]*offset;
  for i in 1..3{let r=1.+normals[i-1].x*normals[i].x+normals[i-1].y*normals[i].y;if r.abs()<=1e-12{return Err("Reversing source curve offset".into());}shifted[i]+=(offset/r)*(normals[i-1]+normals[i]);}
  let mut split=false;for t in [0.25,0.5,0.75]{let a=cubic_point(q,t);let b=cubic_point(shifted,t);let d=(a-b).length_squared();if (d-offset*offset).abs()>0.25*offset*offset{split=true;break;}
   let mt=1.-t;let aa=mt*mt;let bb=t*mt;let cc=t*t;let n=DVec2::new((q[1].y-q[0].y)*aa+(q[2].y-q[1].y)*bb+(q[3].y-q[2].y)*cc,-(q[1].x-q[0].x)*aa-(q[2].x-q[1].x)*bb-(q[3].x-q[2].x)*cc);let length=n.x.abs()+n.y.abs();if length!=0.&& (n.x*(a.y-b.y)-n.y*(a.x-b.x)).abs()/length>0.25*offset{split=true;break;}}
  if split{if depth>=8||out.len()+stack.len()>=15{return Err("Source curve offset subdivision admission exceeded".into());}let(a,b)=cubic_split(q);stack.push((b,depth+1));stack.push((a,depth+1));}else{out.push(shifted);}
 }Ok(out)
}
pub fn source_closed_curve(path:&ShapePath)->Option<ingetrazo_render::qt_gray::PathContour>{
 use ingetrazo_render::qt_gray::{PathContour,Segment};let k=0.5522847498;let pt=|p:DVec2|[p.x,p.y];
 match path{
 ShapePath::Ellipse{size}=>{let c=*size*0.5;let rx=c.x;let ry=c.y;Some(PathContour{start:pt(c+DVec2::new(rx,0.)),segments:vec![
 Segment::Cubic{control1:pt(c+DVec2::new(rx,ry*k)),control2:pt(c+DVec2::new(rx*k,ry)),to:pt(c+DVec2::new(0.,ry))},
 Segment::Cubic{control1:pt(c+DVec2::new(-rx*k,ry)),control2:pt(c+DVec2::new(-rx,ry*k)),to:pt(c-DVec2::new(rx,0.))},
 Segment::Cubic{control1:pt(c+DVec2::new(-rx,-ry*k)),control2:pt(c+DVec2::new(-rx*k,-ry)),to:pt(c-DVec2::new(0.,ry))},
 Segment::Cubic{control1:pt(c+DVec2::new(rx*k,-ry)),control2:pt(c+DVec2::new(rx,-ry*k)),to:pt(c+DVec2::new(rx,0.))}]})},
 ShapePath::RoundedRect{size,radius}if *radius>0.=>{let r=*radius;let w=size.x;let h=size.y;Some(PathContour{start:[r,0.],segments:vec![
 Segment::Line([w-r,0.]),
 Segment::Cubic{control1:[w-r+r*k,0.],control2:[w,r-r*k],to:[w,r]},Segment::Line([w,h-r]),
 Segment::Cubic{control1:[w,h-r+r*k],control2:[w-r+r*k,h],to:[w-r,h]},Segment::Line([r,h]),
 Segment::Cubic{control1:[r-r*k,h],control2:[0.,h-r+r*k],to:[0.,h-r]},Segment::Line([0.,r]),Segment::Cubic{control1:[0.,r-r*k],control2:[r-r*k,0.],to:[r,0.]}]})},_=>None
 }
}
pub fn source_curve_stroke(path:&ingetrazo_render::qt_gray::PathContour,r:f64)->Result<Vec<ingetrazo_render::qt_gray::PathContour>,String>{
 if !r.is_finite()||r<=0.||path.segments.is_empty(){return Err("Invalid source closed curve stroke admission".into());}
 use ingetrazo_render::qt_gray::{PathContour,Segment};let point=|p:[f64;2]|DVec2::new(p[0],p[1]);let pt=|p:DVec2|[p.x,p.y];let mut edges=Vec::new();let mut current=point(path.start);
 for s in &path.segments{let edge=match s{Segment::Line(to)=>{let to=point(*to);let d=to-current;if d.length()<=1e-12{current=to;continue;}let n=DVec2::new(d.y,-d.x)/d.length();vec![[current+n*r,current+n*r,to+n*r,to+n*r]]},Segment::Cubic{control1,control2,to}=>shifted_cubic([current,point(*control1),point(*control2),point(*to)],r)?};current=match s{Segment::Line(to)|Segment::Cubic{to,..}=>point(*to)};edges.extend(edge);}
 let contour=|edges:Vec<Cubic>|{let mut segments=Vec::new();let mut current=edges[0][0];let start=pt(current);for q in edges{if current!=q[0]{segments.push(Segment::Line(pt(q[0])));}if q[0]==q[1]&&q[2]==q[3]{segments.push(Segment::Line(pt(q[3])));}else{segments.push(Segment::Cubic{control1:pt(q[1]),control2:pt(q[2]),to:pt(q[3])});}current=q[3];}PathContour{start,segments}};
 let outer=contour(edges);let mut reversed=path.clone();let mut original=Vec::new();let mut current=point(path.start);for s in &path.segments{let q=match s{Segment::Line(to)=>[current,current,point(*to),point(*to)],Segment::Cubic{control1,control2,to}=>[current,point(*control1),point(*control2),point(*to)]};current=q[3];original.push(q);}original.reverse();reversed.start=pt(original[0][3]);reversed.segments=original.iter().map(|q|if q[0]==q[1]&&q[2]==q[3]{Segment::Line(pt(q[0]))}else{Segment::Cubic{control1:pt(q[2]),control2:pt(q[1]),to:pt(q[0])}}).collect();
 // Inner offsets are shifted along the reversed path's outward normal.
 let mut edges=Vec::new();let mut current=point(reversed.start);for s in &reversed.segments{match s{Segment::Line(to)=>{let to=point(*to);let d=to-current;if d.length()<=1e-12{current=to;continue;}let n=DVec2::new(d.y,-d.x)/d.length();edges.push([current+n*r,current+n*r,to+n*r,to+n*r]);current=to;},Segment::Cubic{control1,control2,to}=>{edges.extend(shifted_cubic([current,point(*control1),point(*control2),point(*to)],r)?);current=point(*to);}}}Ok(vec![outer,contour(edges)])
}
fn round_cap_outline(a:DVec2,b:DVec2,radius:f64)->Option<ingetrazo_render::qt_gray::PathContour>{
 use ingetrazo_render::qt_gray::{PathContour,Segment};let d=b-a;let length=d.length();if length<=1e-12||radius<=0.{return None;}
 let u=d/length*radius;let n=DVec2::new(u.y,-u.x);let k=0.5522847498;
 let point=|p:DVec2|[p.x,p.y];
 Some(PathContour{start:point(a+n),segments:vec![Segment::Line(point(b+n)),
 Segment::Cubic{control1:point(b+n+u*k),control2:point(b+u+n*k),to:point(b+u)},
 Segment::Cubic{control1:point(b+u-n*k),control2:point(b-n+u*k),to:point(b-n)},Segment::Line(point(a-n)),
 Segment::Cubic{control1:point(a-n-u*k),control2:point(a-u-n*k),to:point(a-u)},
 Segment::Cubic{control1:point(a-u+n*k),control2:point(a+n-u*k),to:point(a+n)}]})
}
fn build(ctx:&egui::Context,shape:&SheetShape,zoom:f32,pixels_per_point:f32)->Result<(Arc<Geometry>,Arc<[Batch]>),String>{let(g,b,_)=build_mapped(ctx,shape,zoom,pixels_per_point,DVec2::ZERO)?;Ok((g,b))}
#[derive(Default)]pub struct Cache{shape:Option<SheetShape>,density:u32,phase:[u64;2],geometry:Option<Arc<Geometry>>,batches:Option<Arc<[Batch]>>,mapping:Vec<(usize,bool)>}
fn same_geometry(a:&SheetShape,b:&SheetShape)->bool{
 a.kind==b.kind&&a.w_mm==b.w_mm&&a.h_mm==b.h_mm&&a.stroke_mm==b.stroke_mm&&a.fill==b.fill&&a.invert==b.invert&&a.radius_mm==b.radius_mm&&a.sides==b.sides&&a.ground==b.ground&&a.tick_mm==b.tick_mm&&a.tick_step_mm==b.tick_step_mm&&a.band_mm==b.band_mm
}
impl Cache{
 /// Stable pose/palette changes retain immutable geometry identity. Warm calls
 /// clone only two Arcs; no path generation, tessellation or palette allocation.
 pub fn prepare(&mut self,ctx:&egui::Context,shape:&SheetShape,zoom:f32,pixels_per_point:f32)->Result<(Arc<Geometry>,Arc<[Batch]>),String>{
  self.prepare_at(ctx,shape,zoom,pixels_per_point,Pos2::ZERO)
 }
 pub fn prepare_at(&mut self,ctx:&egui::Context,shape:&SheetShape,zoom:f32,pixels_per_point:f32,translation:Pos2)->Result<(Arc<Geometry>,Arc<[Batch]>),String>{
  if !zoom.is_finite()||zoom<=0.||!pixels_per_point.is_finite()||pixels_per_point<=0.||!(zoom*pixels_per_point).is_finite(){return Err("Invalid composer CAD scale".into());}
  if !translation.x.is_finite()||!translation.y.is_finite(){return Err("Invalid composer physical pixel phase".into());}
  let physical=DVec2::new(f64::from(translation.x)*f64::from(pixels_per_point),f64::from(translation.y)*f64::from(pixels_per_point));
  // Only reuse integer translations where Qt qRound cannot cross its negative
  // half-tie asymmetry. Negative screen coordinates retain complete origin.
  let exact_rect=shape.kind=="rect"&&shape.radius_mm==0.||shape.kind=="poligono";
  let extent=shape.stroke_mm*f64::from(zoom)*f64::from(pixels_per_point)/2.;
  let phase=if !exact_rect{physical}else if physical.min_element()>extent+2. {DVec2::new(physical.x.fract(),physical.y.fract())}else{physical};
  let phase_key=[phase.x.to_bits(),phase.y.to_bits()];
  let density=(zoom*pixels_per_point).to_bits();
  let geometry_matches=self.shape.as_ref().is_some_and(|s|same_geometry(s,shape))&&self.density==density&&self.phase==phase_key;
  if !geometry_matches{
   let(g,b,m)=build_mapped(ctx,shape,zoom,pixels_per_point,phase)?;
   self.geometry=Some(g);self.batches=Some(b);self.mapping=m;self.shape=Some(shape.clone());self.density=density;self.phase=phase_key;
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
#[cfg(any(target_os="linux",target_os="windows"))]
#[derive(Default)]pub struct NativeComposer{
 pub factory:Option<crate::composer_backend::Factory>,
 entries:std::collections::HashMap<uuid::Uuid,(Cache,Option<crate::composer_backend::Handle>)>,
 active:std::collections::HashSet<uuid::Uuid>,
 preview:(Cache,Option<crate::composer_backend::Handle>),
}
#[cfg(any(target_os="linux",target_os="windows"))]
impl NativeComposer{
 pub fn clear_preview(&mut self){self.preview=Default::default();}
 pub fn paint_preview(&mut self,painter:&egui::Painter,shape:&SheetShape,zoom:f32,origin:Pos2)->Result<bool,String>{
  let Some(factory)=&self.factory else{return Ok(false);};
  let(g,b)=self.preview.0.prepare_at(painter.ctx(),shape,zoom,painter.ctx().pixels_per_point(),egui::pos2(origin.x+shape.x_mm as f32*zoom,origin.y+shape.y_mm as f32*zoom))?;
  let pose=ingetrazo_render::cad2d::Pose{scale:[zoom;2],translation:[origin.x+shape.x_mm as f32*zoom,origin.y+shape.y_mm as f32*zoom]};
  if let Some(handle)=&self.preview.1{handle.update(g,b,pose)?;}else{self.preview.1=Some(factory.handle(g,b,pose)?);}
  painter.add(self.preview.1.as_ref().unwrap().shape(painter.clip_rect()));Ok(true)
 }
 pub fn retain_items(&mut self,items:&[ingetrazo_core::composition::SheetItem]){
  self.active.clear();self.active.extend(items.iter().filter_map(|i|if let ingetrazo_core::composition::SheetItem::Shape(s)=i{Some(s.id)}else{None}));
  self.entries.retain(|id,_|self.active.contains(id));
 }
 pub fn paint(&mut self,painter:&egui::Painter,id:uuid::Uuid,shape:&SheetShape,zoom:f32,origin:Pos2)->Result<bool,String>{
  let Some(factory)=&self.factory else{return Ok(false);};
  let (cache,handle)=self.entries.entry(id).or_default();
  let(g,b)=cache.prepare_at(painter.ctx(),shape,zoom,painter.ctx().pixels_per_point(),egui::pos2(origin.x+shape.x_mm as f32*zoom,origin.y+shape.y_mm as f32*zoom))?;
  let pose=ingetrazo_render::cad2d::Pose{scale:[zoom;2],translation:[origin.x+shape.x_mm as f32*zoom,origin.y+shape.y_mm as f32*zoom]};
  if let Some(handle)=handle{handle.update(g,b,pose)?;}else{*handle=Some(factory.handle(g,b,pose)?);}
  painter.add(handle.as_ref().unwrap().shape(painter.clip_rect()));Ok(true)
 }
}
// Metal precise CAD callback routing is not yet installed. The existing
// application mesh path remains generic Color32 and cannot claim source AA.
#[cfg(target_os="macos")]
#[derive(Default)]pub struct NativeComposer;
#[cfg(target_os="macos")]
impl NativeComposer {
 pub fn clear_preview(&mut self){}
 pub fn retain_items(&mut self,_:&[ingetrazo_core::composition::SheetItem]){}
 pub fn paint_preview(&mut self,_:&egui::Painter,_:&SheetShape,_:f32,_:Pos2)->Result<bool,String>{Ok(false)}
 pub fn paint(&mut self,_:&egui::Painter,_:uuid::Uuid,_:&SheetShape,_:f32,_:Pos2)->Result<bool,String>{Ok(false)}
}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn closed_curve_offsets_refuse_degenerate_and_keep_warm_cache(){
  let path=source_closed_curve(&ShapePath::Ellipse{size:DVec2::new(40.,25.)}).unwrap();assert_eq!(source_curve_stroke(&path,0.175).unwrap().len(),2);assert!(source_curve_stroke(&path,f64::NAN).is_err());assert!(source_curve_stroke(&path,0.).is_err());
  assert!(shifted_cubic([DVec2::ZERO;4],1.).is_err());
  let ctx=egui::Context::default();let _=ctx.run(Default::default(),|_|{});let mut cache=Cache::default();let mut shape=SheetShape::default();shape.kind="elipse".into();
  let(g,b)=cache.prepare_at(&ctx,&shape,8.,1.,egui::pos2(50.25,50.25)).unwrap();let(g2,b2)=cache.prepare_at(&ctx,&shape,8.,1.,egui::pos2(50.25,50.25)).unwrap();assert!(Arc::ptr_eq(&g,&g2)&&Arc::ptr_eq(&b,&b2));
 }
 #[test]fn round_caps_cache_actual_origin_and_refuse_degenerate(){
  assert!(round_cap_outline(DVec2::ZERO,DVec2::ZERO,1.).is_none());
  let ctx=egui::Context::default();let _=ctx.run(Default::default(),|_|{});let mut cache=Cache::default();let mut shape=SheetShape::default();shape.kind="linea".into();
  let(g,b)=cache.prepare_at(&ctx,&shape,8.,1.,egui::pos2(50.25,50.25)).unwrap();
  let(g2,b2)=cache.prepare_at(&ctx,&shape,8.,1.,egui::pos2(50.25,50.25)).unwrap();assert!(Arc::ptr_eq(&g,&g2)&&Arc::ptr_eq(&b,&b2));
  let(g3,_)=cache.prepare_at(&ctx,&shape,8.,1.,egui::pos2(50.5,50.25)).unwrap();assert!(!Arc::ptr_eq(&g,&g3));
 }
 #[test]fn bevel_orientation_and_invalid_contours(){
  let points=vec![DVec2::new(0.,0.),DVec2::new(10.,0.),DVec2::new(10.,8.),DVec2::new(0.,8.)];
  let mut reversed=points.clone();reversed.reverse();
  let a=convex_bevel_outline(&points,1.).unwrap();let b=convex_bevel_outline(&reversed,1.).unwrap();
  assert_eq!(a.iter().map(Vec::len).collect::<Vec<_>>(),b.iter().map(Vec::len).collect::<Vec<_>>());
  let concave=vec![DVec2::ZERO,DVec2::new(10.,0.),DVec2::new(3.,3.),DVec2::new(10.,8.),DVec2::new(0.,8.)];
  assert!(convex_bevel_outline(&concave,1.).is_err());assert!(convex_bevel_outline(&points,f64::NAN).is_err());
 }
 #[test]fn pose_palette_and_invalid_updates_preserve_cached_geometry(){
  let ctx=egui::Context::default();let _=ctx.run(Default::default(),|_|{});let mut cache=Cache::default();let mut shape=SheetShape::default();shape.fill=true;
  let(g,b)=cache.prepare(&ctx,&shape,4.,1.).unwrap();shape.x_mm+=17.;shape.z+=1.;
  let(g2,b2)=cache.prepare(&ctx,&shape,4.,1.).unwrap();assert!(Arc::ptr_eq(&g,&g2)&&Arc::ptr_eq(&b,&b2));
  shape.fill_color="#80255d9d".into();let(g3,b3)=cache.prepare(&ctx,&shape,4.,1.).unwrap();assert!(Arc::ptr_eq(&g,&g3));assert!(!Arc::ptr_eq(&b,&b3));
  shape.fill_color="bad-color".into();assert!(cache.prepare(&ctx,&shape,4.,1.).is_err());assert!(Arc::ptr_eq(cache.geometry.as_ref().unwrap(),&g));assert!(Arc::ptr_eq(cache.batches.as_ref().unwrap(),&b3));
 }
 #[test]fn exact_rectangle_phase_cache_and_admission_are_atomic(){
  let ctx=egui::Context::default();let _=ctx.run(Default::default(),|_|{});let mut cache=Cache::default();let mut shape=SheetShape::default();
  let(g,b)=cache.prepare_at(&ctx,&shape,8.,1.,egui::pos2(50.25,50.25)).unwrap();
  let(g2,b2)=cache.prepare_at(&ctx,&shape,8.,1.,egui::pos2(70.25,90.25)).unwrap();assert!(Arc::ptr_eq(&g,&g2)&&Arc::ptr_eq(&b,&b2));
  let(g3,_)=cache.prepare_at(&ctx,&shape,8.,1.,egui::pos2(70.5,90.25)).unwrap();assert!(!Arc::ptr_eq(&g,&g3));
  let(g4,_)=cache.prepare_at(&ctx,&shape,8.,1.,egui::pos2(70.5,90.25)).unwrap();assert!(Arc::ptr_eq(&g3,&g4));
  shape.w_mm=1_000_000.;assert!(cache.prepare_at(&ctx,&shape,8.,1.,egui::pos2(70.5,90.25)).is_err());assert!(Arc::ptr_eq(cache.geometry.as_ref().unwrap(),&g4));
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
