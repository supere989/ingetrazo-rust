// SPDX-License-Identifier: GPL-3.0-or-later
//! Direct physical-page vector output. Caller supplies validated item painters;
//! no rendering process, raster screenshot, or intermediate image is required.
use pdf_writer::{Content, Name, Pdf, Rect, Ref};
use std::{io::Write, path::Path};
#[derive(Clone,Debug)]
pub enum PathOp { Move([f32;2]), Line([f32;2]), Cubic([f32;6]), Close }
#[derive(Clone,Debug)]
pub struct Draw {
 pub path:Vec<PathOp>,
 pub stroke:Option<([f32;4],f32)>,
 pub fill:Option<[f32;4]>,
 pub clip:Option<Vec<PathOp>>,
}
pub struct Page { pub size_mm:[f32;2], pub origin_mm:[f32;2], pub draws:Vec<Draw> }
fn path(content:&mut Content,ops:&[PathOp]) {
 for op in ops {match *op {
  PathOp::Move([x,y])=>{content.move_to(x,y);},
  PathOp::Line([x,y])=>{content.line_to(x,y);},
  PathOp::Cubic([a,b,c,d,e,f])=>{content.cubic_to(a,b,c,d,e,f);},
  PathOp::Close=>{content.close_path();},
 }}
}
fn validate_path(ops:&[PathOp])->Result<(),String> {
 if ops.len()>1_000_000 {return Err("PDF path command budget exceeded".into());}
 let mut open=false;
 for op in ops {
  let points:&[f32]=match op {PathOp::Move(p)=>{open=true;p},PathOp::Line(p)=>{if !open{return Err("PDF path starts without Move".into());}p},PathOp::Cubic(p)=>{if !open{return Err("PDF curve starts without Move".into());}p},PathOp::Close=>{if !open{return Err("PDF close starts without Move".into());}&[]}};
  if points.iter().any(|p|!p.is_finite()||p.abs()>1_000_000.) {return Err("PDF path coordinate exceeds domain".into());}
 }
 Ok(())
}
/// Validates every page before authoring. Millimetres use top-left coordinates.
/// Origin is explicit because original PDF and image export have different margins.
pub fn encode(pages:&[Page])->Result<Vec<u8>,String> {
 if pages.is_empty()||pages.len()>10_000 {return Err("PDF requires 1..10000 pages".into());}
 let mut total=0usize;let mut path_commands=0usize;
 for page in pages {
  if page.size_mm.iter().any(|v|!v.is_finite()||*v<=0.||*v>10_000.)||page.origin_mm.iter().any(|v|!v.is_finite()||v.abs()>10_000.) {return Err("Invalid physical PDF page extent/origin".into());}
  total=total.checked_add(page.draws.len()).ok_or("PDF draw count overflow")?;
  if total>100_000 {return Err("PDF draw budget exceeded".into());}
  for draw in &page.draws {
   path_commands=path_commands.checked_add(draw.path.len()).and_then(|n|n.checked_add(draw.clip.as_ref().map_or(0,Vec::len))).ok_or("PDF command count overflow")?;
   if path_commands>500_000 {return Err("PDF aggregate command budget exceeded".into());}
   validate_path(&draw.path)?;if let Some(clip)=&draw.clip {validate_path(clip)?;}
   for color in draw.fill.iter().chain(draw.stroke.iter().map(|(c,_)|c)) {if color.iter().any(|v|!v.is_finite()||!(0. ..=1.).contains(v)) {return Err("Invalid PDF encoded color".into());}}
   if let Some((_,width))=draw.stroke {if !width.is_finite()||width<=0.||width>10_000. {return Err("Invalid physical PDF stroke width".into());}}
  }
 }
 let mut pdf=Pdf::new();let catalog=Ref::new(1);let tree=Ref::new(2);let mut next=3i32;let mut ids=Vec::new();
 pdf.catalog(catalog).pages(tree);
 for page in pages {
  let id=Ref::new(next);let stream=Ref::new(next+1);next+=2;ids.push(id);
  let alpha_ids=page.draws.iter().map(|_|{let id=Ref::new(next);next+=1;id}).collect::<Vec<_>>();
  let names=(0..page.draws.len()).map(|i|format!("G{i}")).collect::<Vec<_>>();
  let mut content=Content::new();let unit=72./25.4;
  content.transform([unit,0.,0.,-unit,page.origin_mm[0]*unit,(page.size_mm[1]-page.origin_mm[1])*unit]);
  for (i,draw) in page.draws.iter().enumerate() {
   content.save_state();content.set_parameters(Name(names[i].as_bytes()));
   if let Some(clip)=&draw.clip {path(&mut content,clip);content.clip_nonzero();content.end_path();}
   if let Some(color)=draw.fill {content.set_fill_rgb(color[0],color[1],color[2]);}
   if let Some((color,width))=draw.stroke {content.set_stroke_rgb(color[0],color[1],color[2]);content.set_line_width(width);content.set_line_cap(pdf_writer::types::LineCapStyle::RoundCap);content.set_line_join(pdf_writer::types::LineJoinStyle::BevelJoin);}
   path(&mut content,&draw.path);
   match (draw.fill.is_some(),draw.stroke.is_some()) {(true,true)=>{content.fill_nonzero_and_stroke();},(true,false)=>{content.fill_nonzero();},(false,true)=>{content.stroke();},_=>{content.end_path();}}
   content.restore_state();
  }
  let bytes=content.finish();pdf.stream(stream,&bytes);
  for (i,draw) in page.draws.iter().enumerate() {pdf.ext_graphics(alpha_ids[i]).stroking_alpha(draw.stroke.map_or(1.,|(c,_)|c[3])).non_stroking_alpha(draw.fill.map_or(1.,|c|c[3]));}
  let mut writer=pdf.page(id);writer.parent(tree).media_box(Rect::new(0.,0.,page.size_mm[0]*unit,page.size_mm[1]*unit)).contents(stream);
  let mut resources=writer.resources();let mut states=resources.ext_g_states();for (i,id) in alpha_ids.iter().enumerate(){states.pair(Name(names[i].as_bytes()),*id);}
 }
 pdf.pages(tree).kids(ids).count(pages.len()as i32);
 Ok(pdf.finish())
}
/// Same-directory atomic replacement; invalid input never touches destination.
pub fn save(path:&Path,pages:&[Page])->Result<(),String> {
 let bytes=encode(pages)?;let parent=path.parent().filter(|p|!p.as_os_str().is_empty()).unwrap_or(Path::new("."));
 let temp=parent.join(format!(".ingetrazo-sheet-{}.tmp",uuid::Uuid::new_v4()));
 let result=(||{let mut file=std::fs::OpenOptions::new().write(true).create_new(true).open(&temp).map_err(|e|e.to_string())?;file.write_all(&bytes).map_err(|e|e.to_string())?;file.sync_all().map_err(|e|e.to_string())?;drop(file);std::fs::rename(&temp,path).map_err(|e|e.to_string())})();
 if result.is_err(){let _=std::fs::remove_file(temp);}result
}
