//! Original physical sheet border, painted above sheet items through cached native spans.
use std::sync::Arc;
use ingetrazo_render::{qt_gray::{PathContour,Segment},cad2d::{Geometry,Vertex,Batch,Pose,EncodedRgba}};
use ingetrazo_core::composition::{Sheet,SourcePaperBorder};

fn rounded_rect(rect:[f64;4],r:[f64;2])->PathContour{
 let[x,y,w,h]=rect;let rx=r[0].min(w/2.);let ry=r[1].min(h/2.);let k=0.5522847498;
 if rx==0.||ry==0.{return PathContour{start:[x,y],segments:vec![Segment::Line([x+w,y]),Segment::Line([x+w,y+h]),Segment::Line([x,y+h]),Segment::Line([x,y])]};}
 PathContour{start:[x+rx,y],segments:vec![Segment::Line([x+w-rx,y]),Segment::Cubic{control1:[x+w-rx+rx*k,y],control2:[x+w,y+ry-ry*k],to:[x+w,y+ry]},Segment::Line([x+w,y+h-ry]),Segment::Cubic{control1:[x+w,y+h-ry+ry*k],control2:[x+w-rx+rx*k,y+h],to:[x+w-rx,y+h]},Segment::Line([x+rx,y+h]),Segment::Cubic{control1:[x+rx-rx*k,y+h],control2:[x,y+h-ry+ry*k],to:[x,y+h-ry]},Segment::Line([x,y+ry]),Segment::Cubic{control1:[x,y+ry-ry*k],control2:[x+rx-rx*k,y],to:[x+rx,y]}]}
}
// Qt6.12 QStroker::joinPoints uses absolute QLineF angle difference.
// The right→down wrap produces a literal 270-degree first outer join. Preserve
// that path and its inside-turn focal vertices, rather than healing its overlap.
fn sharp_round_stroke(rect:[f64;4],r:f64)->Vec<PathContour>{
 let[x,y,w,h]=rect;let k=0.5522847498;let mut segments=vec![Segment::Line([x+w,y-r])];
 let mut arc=|cx:f64,cy:f64,first:usize,count:usize|{
  let left=cx-r;let top=cy-r;let right=left+2.*r;let bottom=top+2.*r;let cx=left+r;let cy=top+r;
  let points=[[cx,top],[right,cy],[cx,bottom],[left,cy],[cx,top]];
  let controls=[([cx+r*k,top],[right,cy-r*k]),([right,cy+r*k],[cx+r*k,bottom]),([cx-r*k,bottom],[left,cy+r*k]),([left,cy-r*k],[cx-r*k,top])];
  for i in first..first+count{let i=i%4;segments.push(Segment::Cubic{control1:controls[i].0,control2:controls[i].1,to:points[i+1]});}
 };
 arc(x+w,y,0,3);segments.push(Segment::Line([x+w+r,y]));segments.push(Segment::Line([x+w+r,y+h]));
 // End the closure borrow before emitting following physical lines.
 let mut outer=PathContour{start:[x,y-r],segments};
 let append=|path:&mut PathContour,cx:f64,cy:f64,first:usize|{let left=cx-r;let top=cy-r;let right=left+2.*r;let bottom=top+2.*r;let cx=left+r;let cy=top+r;let points=[[cx,top],[right,cy],[cx,bottom],[left,cy],[cx,top]];let controls=[([cx+r*k,top],[right,cy-r*k]),([right,cy+r*k],[cx+r*k,bottom]),([cx-r*k,bottom],[left,cy+r*k]),([left,cy-r*k],[cx-r*k,top])];path.segments.push(Segment::Cubic{control1:controls[first].0,control2:controls[first].1,to:points[first+1]});};
 append(&mut outer,x+w,y+h,1);outer.segments.push(Segment::Line([x,y+h+r]));append(&mut outer,x,y+h,2);outer.segments.push(Segment::Line([x-r,y]));append(&mut outer,x,y,3);
 let inner=PathContour{start:[x+r,y],segments:[[x+r,y+h],[x,y+h],[x,y+h-r],[x+w,y+h-r],[x+w,y+h],[x+w-r,y+h],[x+w-r,y],[x+w,y],[x+w,y+r],[x,y+r],[x,y],[x+r,y]].into_iter().map(Segment::Line).collect()};
 vec![outer,inner]
}
/// Original RoundJoin rectangle stroke. Smooth rounded corners use Qt's source
/// QBezier offset implementation; sharp convex joins use quarter-circle cubics.
pub fn solid_stroke_paths(rect:[f64;4],width:f64,radius:f64)->Result<Vec<PathContour>,String>{
 if rect.iter().chain([width,radius].iter()).any(|v|!v.is_finite())||rect[2]<=0.||rect[3]<=0.||width<=0.||radius<0.||width>=rect[2].min(rect[3]){return Err("Unsupported/invalid sheet border stroke admission".into());}
 let[x,y,w,h]=rect;let r=width/2.;
 if radius>0.01{return crate::composer_cad::source_curve_stroke(&rounded_rect(rect,[radius;2]),r);}
 Ok(sharp_round_stroke([x,y,w,h],r))
}
fn quarter_arc(out:&mut Vec<Segment>,center:[f64;2],r:f64,first:usize,count:usize){
 let[x,y]=center;let left=x-r;let top=y-r;let right=left+2.*r;let bottom=top+2.*r;let x=left+r;let y=top+r;let k=0.5522847498;
 let ends=[[x,top],[right,y],[x,bottom],[left,y],[x,top]];let controls=[([x+r*k,top],[right,y-r*k]),([right,y+r*k],[x+r*k,bottom]),([x-r*k,bottom],[left,y+r*k]),([left,y-r*k],[x-r*k,top])];
 for i in first..first+count{let i=i%4;out.push(Segment::Cubic{control1:controls[i].0,control2:controls[i].1,to:ends[i+1]});}
}
fn axis_open_stroke(points:&[[f64;2]],r:f64)->Result<PathContour,String>{
 // QPointF equality is QtPrivate::fuzzyCompare, including zero operands.
 // Dash carry can produce a sub-ulp corner segment: the source stroker skips
 // it while retaining its previous coordinate for the next normal vector.
 let equal=|a:f64,b:f64|if a==0.||b==0.{(a-b).abs()<=1e-12}else{(a-b).abs()*1e12<=a.abs().min(b.abs())};
 let mut filtered=Vec::new();for p in points{if filtered.last().is_none_or(|a:&[f64;2]|!equal(a[0],p[0])||!equal(a[1],p[1])){filtered.push(*p);}}
 let points=filtered.as_slice();if points.len()<2{return Ok(PathContour{start:points.first().copied().unwrap_or([0.;2]),segments:Vec::new()});}
 let direction=|a:[f64;2],b:[f64;2]|->Result<([f64;2],f64,usize),String>{let d=[b[0]-a[0],b[1]-a[1]];let len=(d[0]*d[0]+d[1]*d[1]).sqrt();if len<=0.||(!equal(a[0],b[0])&&!equal(a[1],b[1])){return Err("Unsupported dashed curved/degenerate border segment".into());}let u=[d[0]/len,d[1]/len];let angle=if u[0].abs()>u[1].abs(){if u[0]>0.{0}else{180}}else if u[1]>0.{270}else{90};Ok((u,len,angle))};
 let side=|points:&[[f64;2]],out:&mut Vec<Segment>|->Result<(),String>{
  for i in 0..points.len()-1{let(u,_,angle)=direction(points[i],points[i+1])?;let n=[u[1]*r,-u[0]*r];out.push(Segment::Line([points[i+1][0]+n[0],points[i+1][1]+n[1]]));
   if i+2<points.len(){let(v,len,nextangle)=direction(points[i+1],points[i+2])?;let next=[points[i+1][0]+v[1]*r,points[i+1][1]-v[0]*r];let cross=u[0]*v[1]-u[1]*v[0];
    if cross<0.&&len>r{out.push(Segment::Line(points[i+1]));}else if cross!=0.{let first=match angle{0=>0,270=>1,180=>2,_=>3};quarter_arc(out,points[i+1],r,first,angle.abs_diff(nextangle)/90);}
    out.push(Segment::Line(next));
   }
  }Ok(())
 };
 if points.len()<2{return Err("Empty dashed border piece".into());}
 let(first,_,_)=direction(points[0],points[1])?;let start=[points[0][0]+first[1]*r,points[0][1]-first[0]*r];let mut out=Vec::new();side(points,&mut out)?;
 let last=points[points.len()-1];let(end,_,_)=direction(points[points.len()-2],last)?;
 out.extend([Segment::Line([last[0]+(end[1]+end[0])*r,last[1]+(-end[0]+end[1])*r]),Segment::Line([last[0]+(-end[1]+end[0])*r,last[1]+(end[0]+end[1])*r]),Segment::Line([last[0]-end[1]*r,last[1]+end[0]*r])]);
 let reverse=points.iter().rev().copied().collect::<Vec<_>>();side(&reverse,&mut out)?;
 let a=points[0];out.extend([Segment::Line([a[0]+(-first[1]-first[0])*r,a[1]+(first[0]-first[1])*r]),Segment::Line([a[0]+(first[1]-first[0])*r,a[1]+(-first[0]-first[1])*r]),Segment::Line(start)]);
 Ok(PathContour{start,segments:out})
}
// QDashStroker physical [4,2]*penWidth traversal. Parenthesized progress math
// and partial-dash carry follow the source; no per-edge pattern restart.
fn dashed_axis_paths(rect:[f64;4],width:f64)->Result<Vec<PathContour>,String>{
 let[x,y,w,h]=rect;if w<=0.||h<=0.||width<=0.{return Err("Invalid dashed border dimensions".into());}
 let points=[[x,y],[x+w,y],[x+w,y+h],[x,y+h],[x,y]];let pattern=[4.*width,2.*width];let(mut index,mut pos,mut offset,mut begin)=(0usize,0.,0.,0.);let mut moveto=points[0];let mut pieces=Vec::<Vec<[f64;2]>>::new();
 for pair in points.windows(2){let a=pair[0];let b=pair[1];let len=(b[0]-a[0]).abs()+(b[1]-a[1]).abs();let stop=begin+len;let mut done=pos>=stop;
  while !done{if pieces.len()>100_000{return Err("Dashed border work admission exceeded".into());}let carry=offset>0.;let on=index==0;let dpos=(pos+pattern[index])-(offset+begin);if !dpos.is_finite()||dpos<0.{return Err("Dashed border progress invalid".into());}
   let p=if dpos>len{offset=pattern[index]-(dpos-len);pos=stop;done=true;b}else{let t=dpos/len;let p=[a[0]+(b[0]-a[0])*t,a[1]+(b[1]-a[1])*t];pos=dpos+begin;done=pos>=stop;index=(index+1)%2;offset=0.;p};
   if on{if !carry||pieces.is_empty(){pieces.push(vec![moveto]);}if pieces.last().unwrap().last()!=Some(&p){pieces.last_mut().unwrap().push(p);}}moveto=p;
  }begin=stop;
 }
 pieces.into_iter().filter(|p|p.len()>1).map(|p|axis_open_stroke(&p,width/2.)).collect()
}
/// Cold geometry only. The source paints outer stroke first, inner double second.
pub fn source_spans(page:[f64;2],margin:f64,border:&SourcePaperBorder,density:f64,phase:[f64;2])->Result<Vec<Vec<ingetrazo_render::qt_gray::Span>>,String>{
 if !border.on{return Ok(Vec::new());}
 if page.iter().chain([margin,density,phase[0],phase[1],border.mm,border.radius_mm].iter()).any(|v|!v.is_finite())||density<=0.||page.iter().any(|v|*v<=0.||*v>100_000.)||margin<0.{return Err("Invalid sheet border projection".into());}
 let radius=border.radius_mm.max(0.);
 if border.style=="dashed"&&radius>0.01{return Err("Source curved dashed sheet border canvas remains unsupported".into());}
 let width=border.mm.max(0.1);let rect=[margin,margin,page[0]-2.*margin,page[1]-2.*margin];let mut strokes=vec![(rect,width)];
 if border.style=="double"{let inset=width*2.+1.5;strokes.push(([rect[0]+inset,rect[1]+inset,rect[2]-2.*inset,rect[3]-2.*inset],(width*0.5).max(0.1)));}
 strokes.into_iter().map(|(rect,width)|{
  let center=rounded_rect(rect,if radius>0.01{[radius;2]}else{[0.;2]});
  if width*density<=1.{
   let map=|p:[f64;2]|[p[0]*density+phase[0],p[1]*density+phase[1]];let mut path=center;path.start=map(path.start);for s in &mut path.segments{match s{Segment::Line(to)=>*to=map(*to),Segment::Cubic{control1,control2,to}=>{*control1=map(*control1);*control2=map(*control2);*to=map(*to);}}}
   if border.style=="dashed"{ingetrazo_render::qt_cosmetic::rasterize_dashed_path(&path,true,width*density,100_000)}else{ingetrazo_render::qt_cosmetic::rasterize_path(&path,true,width*density,100_000)}
  }else{let paths=if border.style=="dashed"{dashed_axis_paths(rect,width)?}else{solid_stroke_paths(rect,width,radius)?};ingetrazo_render::qt_gray::rasterize_transformed_paths(&paths,density,phase,250_000,100_000)}
 }).collect()
}
#[derive(Clone,Default)]pub struct CanvasBorder{
 source:Option<Option<SourcePaperBorder>>,native:serde_json::Value,numeric:Option<[u64;6]>,geometry:Option<Arc<Geometry>>,batches:Option<Arc<[Batch]>>,handle:Option<crate::composer_backend::Handle>,pub cold_builds:u64,
}
impl CanvasBorder{
 pub fn paint(&mut self,painter:&egui::Painter,factory:Option<&crate::composer_backend::Factory>,sheet:&Sheet,zoom:f32,origin:egui::Pos2)->Result<bool,String>{
  let on=sheet.source_border.as_ref().map(|b|b.on).unwrap_or_else(||sheet.native_document["border"]["on"].as_bool().unwrap_or(false));
  if !on{self.geometry=None;self.batches=None;self.handle=None;self.numeric=None;return Ok(true);}
  let Some(factory)=factory else{return Ok(false);};
  let(w,h)=sheet.dimensions_mm();let ppp=painter.ctx().pixels_per_point();let density=f64::from(zoom)*f64::from(ppp);let phase=[(f64::from(origin.x)*f64::from(ppp)).rem_euclid(1.),(f64::from(origin.y)*f64::from(ppp)).rem_euclid(1.)];
  let numeric=[w.to_bits(),h.to_bits(),sheet.margin_mm.to_bits(),density.to_bits(),phase[0].to_bits(),phase[1].to_bits()];
  let same=self.numeric==Some(numeric)&&self.source.as_ref()==Some(&sheet.source_border)&&(sheet.source_border.is_some()||self.native==sheet.native_document["border"]);
  if !same{
   let border=sheet.effective_source_border();let color=EncodedRgba::from_bytes(ingetrazo_core::sheet_color::parse_sheet_color(&border.color)?,1.)?;
   let spans=source_spans([w,h],sheet.margin_mm,&border,density,phase)?;let mut vertices=Vec::new();let mut indices=Vec::new();let mut batches=Vec::new();let mut draws=Vec::new();
   for stroke in spans{
    let first=indices.len()as u32;for range in ingetrazo_render::qt_cosmetic::disjoint_ranges(&stroke,100_000)?{draws.push(first+range.start as u32*6..first+range.end as u32*6);}
    if vertices.len().checked_add(stroke.len()*4).and_then(|n|n.checked_mul(12)).and_then(|n|n.checked_add((indices.len()+stroke.len()*6)*4)).is_none_or(|n|n>8*1024*1024){return Err("Sheet border geometry exceeds8MiB admission".into());}
    for s in stroke{let base=vertices.len()as u32;for p in [[s.x as f64,s.y as f64],[s.x as f64+s.len as f64,s.y as f64],[s.x as f64+s.len as f64,s.y as f64+1.],[s.x as f64,s.y as f64+1.]]{vertices.push(Vertex{position:[((p[0]-phase[0])/density)as f32,((p[1]-phase[1])/density)as f32],coverage:f32::from(s.coverage)/255.});}indices.extend([base,base+1,base+2,base,base+2,base+3]);}
    let end=indices.len()as u32;if end>first{batches.push(Batch{first,count:end-first,color});}
   }
   let geometry=Geometry::new_with_ordered_draws(vertices,indices,draws)?;geometry.validate_batches(&batches)?;
   self.geometry=Some(geometry);self.batches=Some(batches.into());self.source=Some(sheet.source_border.clone());self.native=if sheet.source_border.is_none(){sheet.native_document["border"].clone()}else{serde_json::Value::Null};self.numeric=Some(numeric);self.cold_builds+=1;
  }
  let pose=Pose{scale:[zoom;2],translation:[origin.x,origin.y]};let g=self.geometry.as_ref().unwrap().clone();let b=self.batches.as_ref().unwrap().clone();
  if let Some(handle)=&self.handle{handle.update(g,b,pose)?;}else{self.handle=Some(factory.handle(g,b,pose)?);}
  painter.add(self.handle.as_ref().unwrap().shape(painter.clip_rect()));Ok(true)
 }
}

impl std::fmt::Debug for CanvasBorder{fn fmt(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result{f.debug_struct("CanvasBorder").field("cold_builds",&self.cold_builds).finish()}}
