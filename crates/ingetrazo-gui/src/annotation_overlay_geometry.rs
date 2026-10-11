//! Ordered physical annotation spans. Coverage is immutable cached geometry;
//! retained GPU depth gates remain separate from this CPU producer.
#[path="annotation_widget_primitives.rs"] mod widget_primitives;
use std::{sync::Arc,collections::{HashMap,VecDeque}};
use ingetrazo_formats::paper_text_layout::LayoutFont;
use glam::DVec2;
use ingetrazo_render::{annotations::{Plan,Primitive,Gate},cad2d::{Geometry,Vertex,Batch,SourceSpan,EncodedRgba},qt_gray::{self,Span,PathContour,Segment}};
use ingetrazo_formats::{annotation_font, paper_font_resolver::Request};
/// Source operator selected before palette premultiplication; immutable with its batch.
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Composition { SpanByte, GlyphRgba64Opaque, GlyphLcd([u8;3]), WidgetTriangle([u32;6]), WidgetLineUnion{strip:[u32;16],bounds:[u32;4]}, WidgetStencilTriangle{triangle:[u32;6],bounds:[u32;4]}, WidgetStencilComposite{bounds:[u32;4]} }
#[derive(Clone,Copy,Debug,PartialEq)]
pub struct Draw {pub gate:Gate,pub composition:Composition,pub straight_rgba:[u8;4]}
impl Draw {pub fn span(gate:Gate)->Self{Self{gate,composition:Composition::SpanByte,straight_rgba:[0;4]}}}
pub struct Mesh {pub geometry:Arc<Geometry>,pub batches:Arc<[Batch]>,pub gates:Arc<[Draw]>}
#[derive(Clone,Copy)]pub struct Limits {pub cells:usize,pub spans:usize,pub geometry_bytes:usize,pub glyphs:usize,pub glyph_bytes:usize}
#[derive(Clone,Copy,PartialEq,Eq,Hash)]struct GlyphKey{font:ingetrazo_formats::paper_font::FontKey,pixels:u32,glyph:u32,density:u64,phase:[u8;2],lcd:bool}
#[derive(Clone,Copy,PartialEq,Eq)]enum GlyphPlacement{Raster,OpenGl}
pub struct Cache {analytic_boundary:bool,alpha_union:bool,stencil_union:bool,glyph_order:VecDeque<GlyphKey>,glyphs:HashMap<GlyphKey,Arc<ingetrazo_formats::paper_text::RasterGlyph>>,glyph_bytes:usize,glyph_builds:u64,fonts:annotation_font::Cache,current:Option<(Arc<Plan>,u64,u64,[i32;4],GlyphPlacement,Arc<Mesh>)>,builds:u64,limits:Limits}
impl Cache {
 pub fn new(fonts:annotation_font::Limits,limits:Limits)->Result<Self,String>{if limits.cells==0||limits.spans==0||limits.geometry_bytes==0||limits.glyphs==0||limits.glyph_bytes==0{return Err("Annotation geometry budget must be positive".into());}Ok(Self{analytic_boundary:true,alpha_union:std::env::var_os("INGETRAZO_ANNOTATION_ALPHA_UNION_PROBE").is_some(),stencil_union:std::env::var_os("INGETRAZO_ANNOTATION_STENCIL_PROBE").is_some(),glyph_order:VecDeque::new(),glyphs:HashMap::new(),glyph_bytes:0,glyph_builds:0,fonts:annotation_font::Cache::new(fonts)?,current:None,builds:0,limits})}
 pub(crate) fn enable_analytic_boundary_for_probe(&mut self){self.analytic_boundary=true;self.current=None;}
 pub fn glyph_stats(&self)->(usize,usize,u64){(self.glyphs.len(),self.glyph_bytes,self.glyph_builds)}
 pub fn stats(&self)->(u64,(usize,usize,u64)){(self.builds,self.fonts.stats())}
 pub(crate) fn leader_metrics(&mut self,text:&str,point_size:i32,dpi:f64)->Result<(f64,f64),String>{
  let size=(f64::from(point_size)*dpi/72.+0.5).floor();
  if !size.is_finite()||!(1. ..=4096.).contains(&size){return Err("Annotation metric size outside admission".into());}
  let request=Request{family:"Sans Serif".into(),bold:true,italic:false,required_chars:vec![]};
  let line=self.fonts.shape(&request,text,size as u32)?;
  Ok((((line.ascent_26_6+63)/64+(line.descent_26_6+63)/64)as f64,(line.advance_26_6 as f64/64.+0.5).floor()))
 }
 /// Configure once from the actual native renderpass capability, before preparing
 /// any source widget operations. Default portable/QImage producers are unchanged.
 pub(crate) fn enable_source_stencil(&mut self){self.stencil_union=true;self.current=None;}
 pub fn prepare(&mut self,plan:Arc<Plan>,density:f32,font_dpi:f64)->Result<Arc<Mesh>,String>{
  self.prepare_clipped(plan,density,font_dpi,[-1_000_000,-1_000_000,1_000_001,1_000_001])
 }
 pub fn prepare_clipped(&mut self,plan:Arc<Plan>,density:f32,font_dpi:f64,clip:[i32;4])->Result<Arc<Mesh>,String>{
  self.prepare_for_target(plan,density,font_dpi,clip,GlyphPlacement::Raster)
 }
 pub(crate) fn prepare_widget_clipped(&mut self,plan:Arc<Plan>,density:f32,font_dpi:f64,clip:[i32;4])->Result<Arc<Mesh>,String>{
  self.prepare_for_target(plan,density,font_dpi,clip,GlyphPlacement::OpenGl)
 }
 fn prepare_for_target(&mut self,plan:Arc<Plan>,density:f32,font_dpi:f64,clip:[i32;4],placement:GlyphPlacement)->Result<Arc<Mesh>,String>{
  let density=f64::from(density);
  if !density.is_finite()||density<=0.||density>40.||!font_dpi.is_finite()||font_dpi<=0.{return Err("Annotation physical/font density outside admission".into());}
  if let Some((old,d,dpi,old_clip,old_placement,mesh))=&self.current{if Arc::ptr_eq(old,&plan)&&*d==density.to_bits()&&*dpi==font_dpi.to_bits()&&*old_clip==clip&&*old_placement==placement{return Ok(mesh.clone());}}
  if plan.queries.len()>u32::MAX as usize||plan.queries.iter().flatten().any(|v|!v.is_finite()){
   return Err("Annotation plan query admission exceeded".into());
  }
  let count=plan.queries.len() as u32;
  for op in &plan.operations{match op.gate{
   Gate::Always=>{},Gate::Point(a) if a<count=>{},Gate::Both(a,b) if a<count&&b<count=>{},
   _=>return Err("Annotation plan references absent query".into()),
  }}
  let mesh=Arc::new(self.build(&plan,density,font_dpi,clip,placement)?);
  self.current=Some((plan,density.to_bits(),font_dpi.to_bits(),clip,placement,mesh.clone()));self.builds+=1;Ok(mesh)
 }
 fn glyph(&mut self,face:&ingetrazo_formats::annotation_font::Face,glyph:u32,density:f64,phase:[u8;2],lcd:bool)->Result<Arc<ingetrazo_formats::paper_text::RasterGlyph>,String>{
  let key=GlyphKey{font:face.key(),pixels:face.pixels(),glyph,density:density.to_bits(),phase,lcd};
  if let Some(image)=self.glyphs.get(&key){return Ok(image.clone());}
  self.glyphs.try_reserve(1).map_err(|_|"Annotation glyph cache allocation failed")?;
  self.glyph_order.try_reserve(1).map_err(|_|"Annotation glyph order allocation failed")?;
  // One bounded cold raster candidate (the provider separately caps each mask)
  // is admitted before eviction. Failed candidates never evict existing masks.
  let image=if lcd{let m=face.rasterize_lcd(glyph,density,phase)?;ingetrazo_formats::paper_text::RasterGlyph{left:m.left,top:m.top,width:m.width,height:m.height,coverage:m.coverage}}else{face.rasterize(glyph,density,phase)?};
  let bytes=image.coverage.len();
  if bytes>self.limits.glyph_bytes{return Err("Annotation retained glyph bytes exceeded".into());}
  let mut remaining_bytes=self.glyph_bytes.checked_add(bytes).ok_or("Annotation glyph byte overflow")?;
  let mut remaining_count=self.glyphs.len().checked_add(1).ok_or("Annotation glyph count overflow")?;
  let pressure=remaining_count>self.limits.glyphs||remaining_bytes>self.limits.glyph_bytes;
  let mut removals=Vec::new();
  if pressure{
   removals.try_reserve_exact(self.glyph_order.len()).map_err(|_|"Annotation glyph eviction admission failed")?;
   for key in &self.glyph_order{
    if remaining_count<=self.limits.glyphs&&remaining_bytes<=self.limits.glyph_bytes{break;}
    if let Some(old)=self.glyphs.get(key).filter(|old|Arc::strong_count(old)==1){
     remaining_bytes-=old.coverage.len();remaining_count-=1;removals.push(*key);
    }
   }
   if remaining_count>self.limits.glyphs||remaining_bytes>self.limits.glyph_bytes{
    return Err("Annotation live glyph ownership exceeds admission".into());
   }
  }
  let image=Arc::new(image);
  if pressure{
   for key in removals{
    let old=self.glyphs.remove(&key).expect("admitted cache-only glyph eviction");
    self.glyph_bytes-=old.coverage.len();
   }
   self.glyph_order.retain(|key|self.glyphs.contains_key(key));
  }
  self.glyphs.insert(key,image.clone());self.glyph_order.push_back(key);
  self.glyph_bytes=remaining_bytes;self.glyph_builds+=1;Ok(image)
 }
 fn build(&mut self,plan:&Plan,density:f64,font_dpi:f64,clip:[i32;4],placement:GlyphPlacement)->Result<Mesh,String>{
  let mut writer=Writer::new(density,self.limits);
  for operation in &plan.operations {
   match &operation.primitive {
    Primitive::Line{a,b,width,color}=>{
     if placement==GlyphPlacement::OpenGl&&self.stencil_union&&color[3]!=255{
      if let Some(v)=widget_primitives::square_cap_line(*a,*b,*width)?{let extra=if *width==0.{0.5/density}else{width/2.};let bounds=[(a[0].min(b[0])-extra)as f32,(a[1].min(b[1])-extra)as f32,(a[0].max(b[0])+extra)as f32,(a[1].max(b[1])+extra)as f32];writer.stencil_union(&v,false,bounds,*color,operation.gate)?;}continue;
     }
     if placement==GlyphPlacement::OpenGl&&color[3]==255{if let Some(v)=widget_primitives::square_cap_line(*a,*b,*width)?{writer.direct(&v,&[0,1,2,2,1,3,2,3,4,4,3,5,4,5,6,6,5,7],*color,operation.gate)?;}continue;}
     if placement==GlyphPlacement::OpenGl&&self.alpha_union{
      if let Some(v)=widget_primitives::square_cap_line(*a,*b,*width)?{
       // Qt's nonopaque strip is stencilled, then composited through this
       // original qreal path-control rectangle. Square corners outside it
       // intentionally do not paint, unlike the opaque strip path.
       let extra=if *width==0.{0.5/density}else{width/2.};
       let min=[(a[0].min(b[0])-extra)as f32,(a[1].min(b[1])-extra)as f32];
       let max=[(a[0].max(b[0])+extra)as f32,(a[1].max(b[1])+extra)as f32];
       writer.direct_quad(&[[min[0],min[1]],[max[0],min[1]],[min[0],max[1]],[max[0],max[1]]],*color,operation.gate)?;
       writer.gates.last_mut().unwrap().composition=Composition::WidgetLineUnion{strip:std::array::from_fn(|i|v[i/2][i%2].to_bits()),bounds:[min[0].to_bits(),min[1].to_bits(),max[0].to_bits(),max[1].to_bits()]};
      }continue;
     }
     let path=PathContour{start:*a,segments:vec![Segment::Line(*b)]};
     let spans=if width*density<=1.{let path=transform_path(&path,density);ingetrazo_render::qt_cosmetic::rasterize_path(&path,false,width*density,self.limits.spans)?}
      else{ingetrazo_render::qt_line::rasterize(a.map(|v|v*density),b.map(|v|v*density),width*density,clip,self.limits.cells,self.limits.spans)?};
     writer.layer(spans,*color,operation.gate)?;
    }
    Primitive::Polygon{points,color}=>{
     if placement==GlyphPlacement::OpenGl&&self.stencil_union&&color[3]!=255{
      let v=points.map(|p|p.map(|x|x as f32));let bounds=[v.iter().map(|p|p[0]).fold(f32::INFINITY,f32::min),v.iter().map(|p|p[1]).fold(f32::INFINITY,f32::min),v.iter().map(|p|p[0]).fold(f32::NEG_INFINITY,f32::max),v.iter().map(|p|p[1]).fold(f32::NEG_INFINITY,f32::max)];
      writer.stencil_union(&v,true,bounds,*color,operation.gate)?;continue;
     }
     if placement==GlyphPlacement::OpenGl&&color[3]==255{let v=points.map(|p|p.map(|v|v as f32));
      if self.analytic_boundary{
       if v.iter().flatten().any(|x|!x.is_finite()){return Err("Widget triangle source coordinates invalid".into());}
       let pad=(0.5/density)as f32;
       let min=[v.iter().map(|p|p[0]).fold(f32::INFINITY,f32::min)-pad,v.iter().map(|p|p[1]).fold(f32::INFINITY,f32::min)-pad];
       let max=[v.iter().map(|p|p[0]).fold(f32::NEG_INFINITY,f32::max)+pad,v.iter().map(|p|p[1]).fold(f32::NEG_INFINITY,f32::max)+pad];
       let vertices=[[min[0],min[1]],[max[0],min[1]],[min[0],max[1]],[max[0],max[1]]];
       writer.direct(&vertices,&[0,1,2,2,1,3],*color,operation.gate)?;
       writer.gates.last_mut().unwrap().composition=Composition::WidgetTriangle([v[0][0].to_bits(),v[0][1].to_bits(),v[1][0].to_bits(),v[1][1].to_bits(),v[2][0].to_bits(),v[2][1].to_bits()]);
      }else{writer.direct(&v,&[0,1,2],*color,operation.gate)?;}continue;}let contour=points.map(|p|[p[0]*density,p[1]*density]);writer.layer(qt_gray::rasterize(&[contour.to_vec()],self.limits.cells,self.limits.spans)?,*color,operation.gate)?;}
    Primitive::Ellipse{center,radius,width,color}=>{
     if placement==GlyphPlacement::OpenGl&&self.stencil_union{
      let fill=widget_primitives::ellipse_fill(*center,*radius,density)?;
      let bounds=[(center[0]-radius[0])as f32,(center[1]-radius[1])as f32,(center[0]+radius[0])as f32,(center[1]+radius[1])as f32];
      writer.stencil_union(&fill,true,bounds,*color,operation.gate)?;
      if *width>0.{let strip=widget_primitives::ellipse_stroke(*center,*radius,*width,density)?;let extra=width/2.;let b=[(center[0]-radius[0]-extra)as f32,(center[1]-radius[1]-extra)as f32,(center[0]+radius[0]+extra)as f32,(center[1]+radius[1]+extra)as f32];writer.stencil_union(&strip,false,b,*color,operation.gate)?;}continue;
     }
     let size=DVec2::from_array(*radius)*2.;let mut path=crate::composer_cad::source_closed_curve(&ingetrazo_core::sheet_shape::ShapePath::Ellipse{size}).ok_or("Annotation ellipse path absent")?;
     translate_path(&mut path,[center[0]-radius[0],center[1]-radius[1]]);
     writer.layer(qt_gray::rasterize_transformed_paths(&[path.clone()],density,[0.;2],self.limits.cells,self.limits.spans)?,*color,operation.gate)?;
     if *width>0.{let spans=if width*density<=1.{ingetrazo_render::qt_cosmetic::rasterize_path(&transform_path(&path,density),true,width*density,self.limits.spans)?}
      else{let paths=crate::composer_cad::source_curve_stroke(&path,width/2.)?;qt_gray::rasterize_transformed_paths(&paths,density,[0.;2],self.limits.cells,self.limits.spans)?};
      writer.layer(spans,*color,operation.gate)?;
     }
    }
    Primitive::Text{position,text,size_pt,color}=>{
     let pixel_size=(f64::from(*size_pt)*font_dpi/72.+0.5).floor();
     if !pixel_size.is_finite()||!(1. ..=4096.).contains(&pixel_size){return Err("Annotation resolved point size outside admission".into());}
     let request=Request{family:"Sans Serif".into(),bold:true,italic:false,required_chars:vec![]};
     let line=self.fonts.shape(&request,text,pixel_size as u32)?;
     for run in &line.runs {
      let mut advance=run.x_26_6;
      for glyph in &run.glyphs {
       let x=position[0]*density+(advance+i64::from(glyph.offset[0]))as f64/64.*density;
       let y=position[1]*density+f64::from(glyph.offset[1])/64.*density;
       let (x,y,phase)=match placement{
        GlyphPlacement::Raster=>device_position(x,y)?,
        GlyphPlacement::OpenGl=>opengl_device_position(*position,[advance+i64::from(glyph.offset[0]),i64::from(glyph.offset[1])],density)?,
       };
       let lcd=placement==GlyphPlacement::OpenGl;
       let image=self.glyph(&run.font,glyph.id,density,phase,lcd)?;
       let left=x.checked_add(i64::from(image.left)).ok_or("Annotation glyph position overflow")?;
       let top=y.checked_sub(i64::from(image.top)).ok_or("Annotation glyph position overflow")?;
       if lcd{
        let glyph_first=writer.indices.len()as u32;let glyph_ranges=writer.ranges.len();
        for row in 0..image.height{
         let mut col=0;
         while col<image.width{
          let n=(row*image.width+col)*3;let mask:[u8;3]=image.coverage[n..n+3].try_into().unwrap();let start=col;
          while col<image.width&&image.coverage[(row*image.width+col)*3..(row*image.width+col)*3+3]==mask{col+=1;}
          if mask==[0;3]{continue;}
          writer.layer_with_composition(vec![Span{x:i32::try_from(left+start as i64).map_err(|_|"LCD glyph x outside domain")?,y:i32::try_from(top+row as i64).map_err(|_|"LCD glyph y outside domain")?,len:u32::try_from(col-start).map_err(|_|"LCD glyph span length overflow")?,coverage:255}],*color,operation.gate,Composition::GlyphLcd(mask))?;
          // Source OpenGL A32 setup ignores constant-color alpha. Keep the
          // authored alpha in Draw, but admit the actual opaque constant palette.
          if let Some(batch)=writer.batches.last_mut(){let mut opaque=*color;opaque[3]=255;batch.color=EncodedRgba::from_bytes(opaque,1.)?;}

         }
        }
        // RGB runs are disjoint within one glyph. Preserve one source layer
        // barrier per glyph, rather than introducing a barrier per color run.
        writer.ranges.truncate(glyph_ranges);
        if writer.indices.len()as u32>glyph_first{writer.ranges.push(glyph_first..writer.indices.len()as u32);}
        advance=advance.checked_add(i64::from(glyph.advance[0])).ok_or("Annotation text advance overflow")?;
        continue;
       }
       let mut spans=Vec::new();
       for row in 0..image.height{
        let mut col=0;
        while col<image.width{let coverage=image.coverage[row*image.width+col];let start=col;while col<image.width&&image.coverage[row*image.width+col]==coverage{col+=1;}if coverage==0{continue;}
         if spans.len()>=self.limits.spans{return Err("Annotation glyph run admission exceeded".into());}spans.try_reserve(1).map_err(|_|"Annotation glyph span allocation failed")?;
         spans.push(Span{x:i32::try_from(left+start as i64).map_err(|_|"Annotation glyph x outside domain")?,y:i32::try_from(top+row as i64).map_err(|_|"Annotation glyph y outside domain")?,len:u32::try_from(col-start).map_err(|_|"Annotation glyph span length overflow")?,coverage});
        }
       }
       writer.layer_with_composition(spans,*color,operation.gate,Composition::GlyphRgba64Opaque)?;
       advance=advance.checked_add(i64::from(glyph.advance[0])).ok_or("Annotation text advance overflow")?;
      }
     }
    }
   }
  }
  writer.finish()
 }
}
// Qt getGlyphPositions converts the logical origin to QFixed before adding
// glyph offsets; drawCachedGlyphs then scales and floors that fixed position.
fn opengl_device_position(origin:[f64;2],offset:[i64;2],density:f64)->Result<(i64,i64,[u8;2]),String>{
 if !origin.iter().all(|v|v.is_finite()&&v.abs()<=1_000_000.)||!density.is_finite()||density<=0.||density>40.{return Err("Annotation widget glyph arithmetic outside domain".into());}
 let fixed=[((origin[0]*64.)as i64).checked_add(offset[0]).ok_or("Annotation widget glyph x overflow")?,((origin[1]*64.)as i64).checked_add(offset[1]).ok_or("Annotation widget glyph y overflow")?];
 let x=fixed[0]as f64/64.*density;let y=fixed[1]as f64/64.*density;
 if !x.is_finite()||!y.is_finite()||x.abs()>1_000_000.||y.abs()>1_000_000.{return Err("Annotation widget glyph physical position outside domain".into());}
 // Cache subpixel position is quantized after scaling. Phase1.0 translates
 // the bitmap by one pixel; normalize it to phase0 with that same translation.
 let phase=(((x*64.)as i64).rem_euclid(64)+1)/16*16;
 Ok((x.floor()as i64+phase/64,y.floor()as i64,[(phase%64)as u8,0]))
}
fn device_position(x:f64,y:f64)->Result<(i64,i64,[u8;2]),String>{device_position_for_target(x,y,GlyphPlacement::Raster)}
fn device_position_for_target(x:f64,y:f64,placement:GlyphPlacement)->Result<(i64,i64,[u8;2]),String>{
 if !x.is_finite()||!y.is_finite()||x.abs()>1_000_000.||y.abs()>1_000_000.{return Err("Annotation glyph physical position outside domain".into());}
 let x=(x*64.)as i64;let y=(y*64.)as i64;
 let phase=((x.rem_euclid(64)+1)/16)*16;
 let y=match placement{GlyphPlacement::Raster=>(y+32).div_euclid(64),GlyphPlacement::OpenGl=>y.div_euclid(64)};
 Ok((x.div_euclid(64)+phase/64,y,[(phase%64)as u8,0]))
}
fn translate_path(path:&mut PathContour,delta:[f64;2]){
 let p=|p:&mut [f64;2]|{p[0]+=delta[0];p[1]+=delta[1];};p(&mut path.start);for segment in &mut path.segments{match segment{Segment::Line(to)=>p(to),Segment::Cubic{control1,control2,to}=>{p(control1);p(control2);p(to);}}}
}
fn transform_path(path:&PathContour,density:f64)->PathContour{
 let mut path=path.clone();let p=|p:&mut [f64;2]|{p[0]*=density;p[1]*=density;};p(&mut path.start);for segment in &mut path.segments{match segment{Segment::Line(to)=>p(to),Segment::Cubic{control1,control2,to}=>{p(control1);p(control2);p(to);}}}path
}
struct Writer {density:f64,limits:Limits,vertices:Vec<Vertex>,indices:Vec<u32>,rows:Vec<SourceSpan>,ranges:Vec<std::ops::Range<u32>>,direct_ranges:Vec<std::ops::Range<u32>>,batches:Vec<Batch>,gates:Vec<Draw>}
impl Writer {
 fn new(density:f64,limits:Limits)->Self{Self{density,limits,vertices:Vec::new(),indices:Vec::new(),rows:Vec::new(),ranges:Vec::new(),direct_ranges:Vec::new(),batches:Vec::new(),gates:Vec::new()}}
 fn stencil_union(&mut self,vertices:&[[f32;2]],fan:bool,bounds:[f32;4],color:[u8;4],gate:Gate)->Result<(),String>{
  if vertices.len()<3||color[3]==0{return Ok(());}
  let pad=(0.5/self.density)as f32;let first_marker=self.batches.len();
  for i in 0..vertices.len()-2{
   let triangle=if fan{[vertices[0],vertices[i+1],vertices[i+2]]}else{[vertices[i],vertices[i+1],vertices[i+2]]};
   // Source strip joins deliberately duplicate vertices. These triangles
   // remain degenerate under every admitted affine pose, so no GPU marker
   // geometry, palette record, push or draw is needed for them.
   if triangle[0]==triangle[1]||triangle[1]==triangle[2]||triangle[0]==triangle[2]{continue;}
   let min=std::array::from_fn::<_,2,_>(|axis|triangle.iter().map(|p|p[axis]).fold(f32::INFINITY,f32::min)-pad);
   let max=std::array::from_fn::<_,2,_>(|axis|triangle.iter().map(|p|p[axis]).fold(f32::NEG_INFINITY,f32::max)+pad);
   let mark=[color[0],color[1],color[2],255];
   self.direct_quad(&[[min[0],min[1]],[max[0],min[1]],[min[0],max[1]],[max[0],max[1]]],mark,gate)?;
   self.gates.last_mut().unwrap().composition=Composition::WidgetStencilTriangle{triangle:std::array::from_fn(|i|triangle[i/2][i%2].to_bits()),bounds:bounds.map(f32::to_bits)};
  }
  if self.batches.len()==first_marker{return Ok(());}
  self.direct_quad(&[[bounds[0]-pad,bounds[1]-pad],[bounds[2]+pad,bounds[1]-pad],[bounds[0]-pad,bounds[3]+pad],[bounds[2]+pad,bounds[3]+pad]],color,gate)?;
  self.gates.last_mut().unwrap().composition=Composition::WidgetStencilComposite{bounds:bounds.map(f32::to_bits)};Ok(())
 }
 fn direct_quad(&mut self,vertices:&[[f32;2];4],color:[u8;4],gate:Gate)->Result<(),String>{self.direct_inner(vertices,&[0,1,2,2,1,3],color,gate,true)}
 fn direct(&mut self,vertices:&[[f32;2]],indices:&[u32],color:[u8;4],gate:Gate)->Result<(),String>{self.direct_inner(vertices,indices,color,gate,false)}
 fn direct_inner(&mut self,vertices:&[[f32;2]],indices:&[u32],color:[u8;4],gate:Gate,union:bool)->Result<(),String>{
  if (!union&&color[3]!=255)||vertices.iter().flatten().any(|v|!v.is_finite())||indices.iter().any(|&i|i as usize>=vertices.len()){return Err("Direct annotation primitive admission failed".into());}
  let vc=self.vertices.len().checked_add(vertices.len()).ok_or("Annotation vertex overflow")?;
  let ic=self.indices.len().checked_add(indices.len()).ok_or("Annotation index overflow")?;
  if vc>u32::MAX as usize||ic>u32::MAX as usize{return Err("Annotation index domain exceeded".into());}
  let bytes=vc.checked_mul(std::mem::size_of::<Vertex>()).and_then(|n|ic.checked_mul(4).and_then(|i|n.checked_add(i)))
   .and_then(|n|self.rows.len().checked_mul(std::mem::size_of::<SourceSpan>()).and_then(|i|n.checked_add(i)))
   .and_then(|n|(self.ranges.len()+self.direct_ranges.len()+2).checked_mul(std::mem::size_of::<std::ops::Range<u32>>()).and_then(|i|n.checked_add(i)))
   .and_then(|n|(self.batches.len()+1).checked_mul(std::mem::size_of::<Batch>()+std::mem::size_of::<Draw>()).and_then(|i|n.checked_add(i)));
  if bytes.is_none_or(|n|n>self.limits.geometry_bytes){return Err("Annotation direct retained admission exceeded".into());}
  self.vertices.try_reserve(vertices.len()).map_err(|_|"Annotation direct vertices allocation failed")?;
  self.indices.try_reserve(indices.len()).map_err(|_|"Annotation direct indices allocation failed")?;
  self.ranges.try_reserve(1).map_err(|_|"Annotation direct range allocation failed")?;
  self.direct_ranges.try_reserve(1).map_err(|_|"Annotation direct proof allocation failed")?;
  self.batches.try_reserve(1).map_err(|_|"Annotation direct batch allocation failed")?;
  self.gates.try_reserve(1).map_err(|_|"Annotation direct gate allocation failed")?;
  let first=self.indices.len()as u32;let base=self.vertices.len()as u32;
  self.vertices.extend(vertices.iter().map(|&position|Vertex{position,coverage:1.}));self.indices.extend(indices.iter().map(|&i|base+i));
  let range=first..self.indices.len()as u32;self.ranges.push(range.clone());self.direct_ranges.push(range.clone());
  self.batches.push(Batch{first,count:range.end-first,color:EncodedRgba::from_bytes(color,1.)?});self.gates.push(Draw{gate,composition:Composition::SpanByte,straight_rgba:color});Ok(())
 }
 fn layer(&mut self,spans:Vec<Span>,color:[u8;4],gate:Gate)->Result<(),String>{self.layer_with_composition(spans,color,gate,Composition::SpanByte)}
 fn layer_with_composition(&mut self,spans:Vec<Span>,color:[u8;4],gate:Gate,composition:Composition)->Result<(),String>{
  if spans.is_empty(){return Ok(());}
  let count=self.rows.len().checked_add(spans.len()).filter(|&n|n<=self.limits.spans&&n<=u32::MAX as usize/6).ok_or("Annotation aggregate span admission exceeded")?;
  let ranges=ingetrazo_render::qt_cosmetic::disjoint_ranges(&spans,self.limits.spans)?;
  let retained=[
   (count.checked_mul(4),std::mem::size_of::<Vertex>()),
   (count.checked_mul(6),std::mem::size_of::<u32>()),
   (Some(count),std::mem::size_of::<SourceSpan>()),
   (self.ranges.len().checked_add(ranges.len()),std::mem::size_of::<std::ops::Range<u32>>()),
   (self.batches.len().checked_add(1),std::mem::size_of::<Batch>()),
   (self.gates.len().checked_add(1),std::mem::size_of::<Draw>()),
  ].into_iter().try_fold(0usize,|sum,(count,size)|count.and_then(|n|n.checked_mul(size)).and_then(|n|sum.checked_add(n)));
  if retained.is_none_or(|bytes|bytes>self.limits.geometry_bytes){return Err("Annotation retained arrays admission exceeded".into());}
  self.vertices.try_reserve(spans.len()*4).map_err(|_|"Annotation vertex allocation failed")?;
  self.indices.try_reserve(spans.len()*6).map_err(|_|"Annotation index allocation failed")?;
  self.rows.try_reserve(spans.len()).map_err(|_|"Annotation row allocation failed")?;
  self.ranges.try_reserve(ranges.len()).map_err(|_|"Annotation order allocation failed")?;
  self.batches.try_reserve(1).map_err(|_|"Annotation batch allocation failed")?;
  self.gates.try_reserve(1).map_err(|_|"Annotation gate allocation failed")?;
  let first=u32::try_from(self.indices.len()).map_err(|_|"Annotation index domain exceeded")?;
  for range in ranges{self.ranges.push(first+range.start as u32*6..first+range.end as u32*6);}
  for span in spans{
   self.rows.push(SourceSpan{x:span.x,y:span.y,len:i32::try_from(span.len).map_err(|_|"Annotation row length exceeded")?,coverage:span.coverage});
   let base=u32::try_from(self.vertices.len()).map_err(|_|"Annotation vertex domain exceeded")?;
   for p in [[span.x as f64,span.y as f64],[span.x as f64+f64::from(span.len),span.y as f64],[span.x as f64+f64::from(span.len),span.y as f64+1.],[span.x as f64,span.y as f64+1.]]{
    self.vertices.push(Vertex{position:[(p[0]/self.density)as f32,(p[1]/self.density)as f32],coverage:f32::from(span.coverage)/255.});
   }
   self.indices.extend([base,base+1,base+2,base,base+2,base+3]);
  }
  self.batches.push(Batch{first,count:self.indices.len()as u32-first,color:EncodedRgba::from_bytes(color,1.)?});self.gates.push(Draw{gate,composition,straight_rgba:color});Ok(())
 }
 fn finish(self)->Result<Mesh,String>{
  let geometry=if self.direct_ranges.is_empty(){Geometry::new_with_source_spans(self.vertices,self.indices,self.ranges,self.rows,1.,self.density as f32,[0.;2])?}else{
   use ingetrazo_render::cad2d::AnnotationLayer;
   let mut layers=Vec::new();layers.try_reserve(self.ranges.len()).map_err(|_|"Annotation layer allocation failed")?;
   let mut rows=self.rows.into_iter();let mut direct=self.direct_ranges.into_iter().peekable();
   let mut batch_index=0;
   for range in self.ranges{
    while self.batches.get(batch_index).is_some_and(|b|b.first+b.count<=range.start){batch_index+=1;}
    if direct.peek()==Some(&range){
     let batch=self.batches.get(batch_index).ok_or("Annotation direct palette absent")?;
     if batch.first!=range.start||batch.first+batch.count!=range.end{return Err("Annotation direct range differs from its palette batch".into());}
     direct.next();layers.push(if matches!(self.gates[batch_index].composition,Composition::WidgetLineUnion{..}|Composition::WidgetStencilComposite{..}){AnnotationLayer::UnionQuad{range}}else{AnnotationLayer::Opaque{range}});}
    else{let n=(range.end-range.start)as usize/6;let mut part=Vec::new();part.try_reserve(n).map_err(|_|"Annotation source layer allocation failed")?;part.extend(rows.by_ref().take(n));if part.len()!=n{return Err("Annotation source rows absent".into());}layers.push(AnnotationLayer::Spans{range,rows:part});}
   }
   if rows.next().is_some()||direct.next().is_some(){return Err("Annotation layer inventory mismatch".into());}
   Geometry::new_with_annotation_layers(self.vertices,self.indices,layers,self.density as f32)?
  };
  Ok(Mesh{geometry,batches:self.batches.into(),gates:self.gates.into()})
 }
}


#[cfg(all(test,target_os="linux"))]
mod tests {
 use super::*;
 use ingetrazo_render::annotations::Operation;
 fn plan()->Arc<Plan>{Arc::new(Plan{queries:vec![[0.;3]],operations:vec![
  Operation{primitive:Primitive::Polygon{points:[[20.,20.],[40.,20.],[20.,40.]],color:[45,55,75,255]},gate:Gate::Point(0)},
  Operation{primitive:Primitive::Text{position:[45.,35.],text:"A".into(),size_pt:11,color:[243,115,41,255]},gate:Gate::Always},
 ]})}
 fn cache()->Cache{Cache::new(annotation_font::Limits{faces:16,retained_font_bytes:32*1024*1024},Limits{cells:250_000,spans:100_000,geometry_bytes:8*1024*1024,glyphs:4096,glyph_bytes:16*1024*1024}).unwrap()}
 #[test]fn mixed_lcd_runs_keep_later_translucent_composite_classification(){
  let mut w=Writer::new(1.,cache().limits);
  for x in 0..3{w.layer_with_composition(vec![Span{x,y:0,len:1,coverage:255}],[0,0,0,255],Gate::Always,Composition::GlyphLcd([128;3])).unwrap();}
  w.ranges.truncate(0);w.ranges.push(0..18);
  w.stencil_union(&[[10.,10.],[20.,10.],[10.,20.]],true,[10.,10.,20.,20.],[37,93,157,128],Gate::Always).unwrap();
  let mesh=w.finish().unwrap();mesh.geometry.validate_batches(&mesh.batches).unwrap();
  assert!(matches!(mesh.gates.last().unwrap().composition,Composition::WidgetStencilComposite{..}));
 }
 #[test]fn stencil_duplicate_vertices_do_not_allocate_or_leave_unmatched_composite(){
  let limits=cache().limits;let mut writer=Writer::new(1.,limits);
  writer.stencil_union(&[[1.,1.];4],false,[0.,0.,10.,10.],[37,93,157,128],Gate::Always).unwrap();
  assert!(writer.vertices.is_empty()&&writer.indices.is_empty()&&writer.gates.is_empty());
  writer.stencil_union(&[[1.,1.],[1.,1.],[2.,1.],[2.,2.]],false,[0.,0.,10.,10.],[37,93,157,128],Gate::Always).unwrap();
  assert_eq!(writer.gates.len(),2);
  assert!(matches!(writer.gates[0].composition,Composition::WidgetStencilTriangle{..}));
  assert!(matches!(writer.gates[1].composition,Composition::WidgetStencilComposite{..}));
 }
 #[test]fn stencil_curve_layers_preserve_two_composites_and_warm_owner(){
  let mut cache=cache();cache.stencil_union=true;
  let plan=Arc::new(Plan{queries:vec![],operations:vec![Operation{primitive:Primitive::Ellipse{center:[80.125,80.375],radius:[37.125,9.875],width:3.125,color:[37,93,157,128]},gate:Gate::Always}]});
  let first=cache.prepare_widget_clipped(plan.clone(),1.,96.,[0,0,640,400]).unwrap();assert_eq!(first.gates.iter().filter(|g|matches!(g.composition,Composition::WidgetStencilComposite{..})).count(),2);
  assert!(first.geometry.validate_batches(&first.batches).is_ok());assert!(!first.geometry.is_opaque(&first.batches));
  assert!(Arc::ptr_eq(&first,&cache.prepare_widget_clipped(plan.clone(),1.,96.,[0,0,640,400]).unwrap()));
  assert!(!Arc::ptr_eq(&first,&cache.prepare_widget_clipped(plan,1.25,96.,[0,0,640,400]).unwrap()));
 }
 #[test]fn alpha_union_retains_one_quad_per_operation_and_warm_identity(){
  let mut cache=cache();cache.alpha_union=true;
  let plan=Arc::new(Plan{queries:vec![],operations:[0,1,127,128,230].into_iter().map(|alpha|Operation{primitive:Primitive::Line{a:[20.25,20.5],b:[180.75,80.25],width:3.125,color:[37,93,157,alpha]},gate:Gate::Always}).collect()});
  let mesh=cache.prepare_widget_clipped(plan.clone(),1.,96.,[0,0,640,400]).unwrap();
  assert_eq!(mesh.geometry.vertices().len(),20);assert_eq!(mesh.geometry.indices().len(),30);
  assert!(mesh.gates.iter().all(|g|matches!(g.composition,Composition::WidgetLineUnion{..})));
  assert!(mesh.geometry.validate_batches(&mesh.batches).is_ok());
  assert!(Arc::ptr_eq(&mesh,&cache.prepare_widget_clipped(plan,1.,96.,[0,0,640,400]).unwrap()));
 }
 #[test]fn immutable_plan_warm_reuses_geometry_and_density_change_invalidates(){
  let mut cache=cache();let plan=plan();let first=cache.prepare(plan.clone(),1.,96.).unwrap();let counts=cache.stats();let glyph_counts=cache.glyph_stats();
  assert!(!first.geometry.source_spans().unwrap().rows().is_empty());
  assert_eq!(first.gates[0].gate,Gate::Point(0));
  for _ in 0..8{let warm=cache.prepare(plan.clone(),1.,96.).unwrap();assert!(Arc::ptr_eq(&first,&warm));assert_eq!(cache.stats(),counts);assert_eq!(cache.glyph_stats(),glyph_counts);}
  let changed=cache.prepare(plan.clone(),1.5,96.).unwrap();assert!(!Arc::ptr_eq(&first,&changed));assert_ne!(first.geometry.id(),changed.geometry.id());
 }
 #[test]fn widget_rgb_masks_are_cached_without_changing_raster_default(){
  let mut cache=cache();let plan=plan();let raster=cache.prepare(plan.clone(),1.,96.).unwrap();
  assert!(raster.gates.iter().all(|g|!matches!(g.composition,Composition::GlyphLcd(_))));
  let lcd=cache.prepare_widget_clipped(plan.clone(),1.,96.,[0,0,800,600]).unwrap();
  assert!(lcd.gates.iter().any(|g|matches!(g.composition,Composition::GlyphLcd(m) if m[0]!=m[1]||m[1]!=m[2])));
  let counts=cache.glyph_stats();let warm=cache.prepare_widget_clipped(plan,1.,96.,[0,0,800,600]).unwrap();
  assert!(Arc::ptr_eq(&lcd,&warm));assert_eq!(counts,cache.glyph_stats());
 }
 #[test]fn glyph_capacity_refusal_keeps_previous_mesh_and_cached_mask(){
  let mut cache=cache();cache.limits.glyphs=1;
  let initial=plan();let first=cache.prepare(initial.clone(),1.,96.).unwrap();
  let old_glyphs=cache.glyph_stats();assert_eq!(old_glyphs.0,1);
  let retained_mask=cache.glyphs.values().next().unwrap().clone();
  let next=Arc::new(Plan{queries:vec![],operations:vec![Operation{
   primitive:Primitive::Text{position:[45.,35.],text:"B".into(),size_pt:11,color:[45,55,75,255]},gate:Gate::Always}]});
  assert!(cache.prepare(next,1.,96.).is_err());
  assert_eq!(cache.glyph_stats(),old_glyphs);
  assert!(Arc::ptr_eq(&first,&cache.prepare(initial,1.,96.).unwrap()));
  drop(retained_mask);
 }
 #[test]fn glyph_phase_churn_evicts_cache_only_masks_and_keeps_warm_mesh(){
  let mut cache=cache();cache.limits.glyphs=2;let plan=plan();
  for density in [1.,1.25,1.5,1.75,2.,2.25,2.5]{
   let mesh=cache.prepare(plan.clone(),density,96.).unwrap();
   assert!(cache.glyph_stats().0<=2);assert!(cache.glyph_order.len()<=2);
   let before=cache.glyph_stats();
   assert!(Arc::ptr_eq(&mesh,&cache.prepare(plan.clone(),density,96.).unwrap()));
   assert_eq!(cache.glyph_stats(),before);
  }
  assert!(cache.glyph_stats().2>2);
 }
 #[test]fn retained_metadata_is_in_geometry_admission(){
  let mut writer=Writer::new(1.,Limits{cells:10,spans:10,geometry_bytes:72,glyphs:1,glyph_bytes:1});
  assert!(writer.layer(vec![Span{x:0,y:0,len:1,coverage:255}],[0,0,0,255],Gate::Always).is_err());
  assert!(writer.vertices.is_empty()&&writer.rows.is_empty());
 }
 #[test]fn malformed_candidate_keeps_last_ready_mesh(){
  let mut cache=cache();let plan=plan();let first=cache.prepare(plan.clone(),1.,96.).unwrap();let counts=cache.stats();
  let malformed=Arc::new(Plan{queries:vec![],operations:vec![Operation{primitive:Primitive::Polygon{points:[[f64::MAX,0.],[1.,1.],[2.,0.]],color:[0,0,0,255]},gate:Gate::Always}]});
  assert!(cache.prepare(malformed,1.,96.).is_err());assert_eq!(cache.stats(),counts);
  let invalid_gate=Arc::new(Plan{queries:vec![],operations:vec![Operation{primitive:Primitive::Text{position:[45.,35.],text:"B".into(),size_pt:11,color:[0,0,0,255]},gate:Gate::Point(0)}]});
  let glyphs=cache.glyph_stats();assert!(cache.prepare(invalid_gate,1.,96.).is_err());assert_eq!(cache.glyph_stats(),glyphs);
  assert!(Arc::ptr_eq(&first,&cache.prepare(plan,1.,96.).unwrap()));
 }
}

#[cfg(test)]mod widget_placement_tests{
 use super::*;
 #[test]fn widget_fixed_origin_precedes_fractional_density(){
  for(y,density,expected)in[(-24.015,1.25,-30),(-24.677,1.5,-38),(0.805,1.25,0)]{
   let actual=opengl_device_position([10.25,y],[0,0],density).unwrap().1;
   let old=device_position_for_target(10.25*density,y*density,GlyphPlacement::OpenGl).unwrap().1;
   assert_eq!(actual,expected);assert_ne!(actual,old);
  }
  assert_eq!(opengl_device_position([0.,-0.001],[0,0],1.).unwrap().1,0);
  assert_eq!(opengl_device_position([0.,-22.001],[0,0],1.).unwrap().1,-22);
  assert!(opengl_device_position([0.,f64::INFINITY],[0,0],1.).is_err());
 }
 #[test]fn widget_baseline_floor_preserves_raster_default(){
  for(y,raster,widget)in[(22.75,23,22),(-22.25,-22,-23),(22.25,22,22)]{
   assert_eq!(device_position(10.25,y).unwrap().1,raster);
   assert_eq!(device_position_for_target(10.25,y,GlyphPlacement::OpenGl).unwrap().1,widget);
  }
 }
}
