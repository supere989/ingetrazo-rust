// Copyright (C) 2016 The Qt Company Ltd.
// Copyright (C) 2000-2016 David Turner, Robert Wilhelm, Werner Lemberg.
// SPDX-License-Identifier: FTL AND GPL-3.0-only
// Rust adaptation of line/cell/sweep mathematics; see licenses/FTL.txt.
//! Bounded native line-outline scan conversion matching Qt6.12 gray raster.
//! Source: Qt qoutlinemapper.cpp (26.6 qRound), qgrayraster.c line/cell/sweep.
//! Curves use source QBezier subdivision before fixed-point cells; stroking is external.
use std::collections::BTreeMap;
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct Span {pub x:i32,pub y:i32,pub len:u32,pub coverage:u8}
struct Cells {map:BTreeMap<(i64,i64),(i64,i64)>,limit:usize,steps:usize}
impl Cells {
 fn add(&mut self,x:i64,y:i64,cover:i64,area:i64)->Result<(),String>{
  self.steps+=1;if self.steps>self.limit.saturating_mul(8){return Err("Gray raster edge-work limit exceeded".into());}
  if cover==0&&area==0{return Ok(());}
  if !self.map.contains_key(&(y,x))&&self.map.len()>=self.limit{return Err("Gray raster cell limit exceeded".into());}
  let c=self.map.entry((y,x)).or_default();c.0+=cover;c.1+=area;Ok(())
 }
 fn scan(&mut self,y:i64,x1:i64,mut y1:i64,x2:i64,y2:i64)->Result<(),String>{
  if y1==y2{return Ok(());}
  let(mut ex,ex2)=(x1>>8,x2>>8);let(mut fx,fx2)=(x1&255,x2&255);
  if ex!=ex2 {
   let dx=x2-x1;let dy=y2-y1;
   let(first,incr,p,den)=if dx>0{(256,1,(256-fx)*dy,dx)}else{(0,-1,fx*dy,-dx)};
   let mut delta=p.div_euclid(den);let mut rem=p.rem_euclid(den);
   self.add(ex,y,delta,(fx+first)*delta)?;y1+=delta;ex+=incr;
   if ex!=ex2 {
    let p=256*dy;let lift=p.div_euclid(den);let carry=p.rem_euclid(den);
    while ex!=ex2 {delta=lift;rem+=carry;if rem>=den{rem-=den;delta+=1;}self.add(ex,y,delta,256*delta)?;y1+=delta;ex+=incr;}
   }
   fx=256-first;
  }
  let dy=y2-y1;self.add(ex,y,dy,(fx+fx2)*dy)
 }
 fn line(&mut self,a:[i64;2],b:[i64;2])->Result<(),String>{
  let(mut ey,ey2)=(a[1]>>8,b[1]>>8);let(fy,fy2)=(a[1]&255,b[1]&255);
  if ey==ey2{return self.scan(ey,a[0],fy,b[0],fy2);}
  let dx=b[0]-a[0];let dy=b[1]-a[1];
  let(first,incr,p,den)=if dy>0{(256,1,(256-fy)*dx,dy)}else{(0,-1,fy*dx,-dy)};
  let delta=p.div_euclid(den);let mut rem=p.rem_euclid(den);let mut x=a[0]+delta;
  self.scan(ey,a[0],fy,x,first)?;ey+=incr;
  if ey!=ey2 {let p=256*dx;let lift=p.div_euclid(den);let carry=p.rem_euclid(den);
   while ey!=ey2 {let mut delta=lift;rem+=carry;if rem>=den{rem-=den;delta+=1;}let next=x+delta;self.scan(ey,x,256-first,next,first)?;x=next;ey+=incr;}
  }
  self.scan(ey,x,256-first,b[0],fy2)
 }
}
fn coverage(area:i64)->u8{(area>>9).unsigned_abs().min(255) as u8}
fn append(out:&mut Vec<Span>,x:i64,y:i64,len:i64,c:u8,limit:usize)->Result<(),String>{
 if c==0||len<=0{return Ok(());}
 let x=i32::try_from(x).map_err(|_|"Gray raster coordinate overflow")?;let y=i32::try_from(y).map_err(|_|"Gray raster coordinate overflow")?;let len=u32::try_from(len).map_err(|_|"Gray raster span overflow")?;
 if let Some(last)=out.last_mut(){if last.y==y&&last.coverage==c&&i64::from(last.x)+i64::from(last.len)==i64::from(x){last.len=last.len.checked_add(len).ok_or("Gray raster run overflow")?;return Ok(());}}
 if out.len()>=limit{return Err("Gray raster run limit exceeded".into());}out.push(Span{x,y,len,coverage:c});Ok(())
}
/// Input is transformed physical pixel coordinates; nonzero winding contours.
/// All work is prepared locally and errors return no partially authored mesh.
pub fn rasterize(contours:&[Vec<[f64;2]>],max_cells:usize,max_runs:usize)->Result<Vec<Span>,String>{
 if max_cells==0||max_runs==0{return Err("Gray raster admission must be positive".into());}
 let edge_count=contours.iter().try_fold(0_usize,|count,c|count.checked_add(c.len())).ok_or("Gray raster edge-count overflow")?;
 if edge_count>max_cells{return Err("Gray raster edge admission exceeded".into());}
 let mut cells=Cells{map:BTreeMap::new(),limit:max_cells,steps:0};
 for contour in contours {
  if contour.len()<3{return Err("Gray raster contour requires three points".into());}
  let fixed=contour.iter().map(|p|{if p.iter().any(|x|!x.is_finite()||x.abs()>1_000_000.){return Err("Gray raster input outside finite bounded coordinates".into());}Ok([(p[0]*64.).round() as i64*4,(p[1]*64.).round() as i64*4])}).collect::<Result<Vec<_>,String>>()?;
  for i in 0..fixed.len(){cells.line(fixed[i],fixed[(i+1)%fixed.len()])?;}
 }
 finish(cells,max_runs)
}
fn finish(cells:Cells,max_runs:usize)->Result<Vec<Span>,String>{
 let mut out=Vec::new();let(mut row,mut cover,mut next)=(None,0_i64,0_i64);
 for((y,x),(dc,area))in cells.map {
  if row!=Some(y){row=Some(y);cover=0;next=x;}
  if x>next{append(&mut out,next,y,x-next,coverage(cover*512),max_runs)?;}
  cover+=dc;append(&mut out,x,y,1,coverage(cover*512-area),max_runs)?;next=x+1;
 }
 Ok(out)
}
#[derive(Clone,Debug)]
pub enum Segment{Line([f64;2]),Cubic{control1:[f64;2],control2:[f64;2],to:[f64;2]}}
#[derive(Clone,Debug)]pub struct PathContour{pub start:[f64;2],pub segments:Vec<Segment>}
fn fixed(p:[f64;2])->Result<[i64;2],String>{
 if p.iter().any(|x|!x.is_finite()||x.abs()>1_000_000.){return Err("Gray raster input outside finite bounded coordinates".into());}
 Ok([(p[0]*64.).round()as i64*4,(p[1]*64.).round()as i64*4])
}
// QBezier::addToPolygon adaptation, Copyright (C) 2016 The Qt Company Ltd.
// Used under GPL-3.0-only; repository LICENSE.
fn flatten_cubic(a:[f64;2],c1:[f64;2],c2:[f64;2],b:[f64;2])->Vec<[f64;2]>{
 let mut stack=vec![([a,c1,c2,b],9usize)];let mut out=Vec::new();
 while let Some((q,level))=stack.pop(){let dx=q[3][0]-q[0][0];let dy=q[3][1]-q[0][1];let mut length=dx.abs()+dy.abs();
  let deviation=if length>1.{(dx*(q[0][1]-q[1][1])-dy*(q[0][0]-q[1][0])).abs()+(dx*(q[0][1]-q[2][1])-dy*(q[0][0]-q[2][0])).abs()}else{length=1.;(q[0][0]-q[1][0]).abs()+(q[0][1]-q[1][1]).abs()+(q[0][0]-q[2][0]).abs()+(q[0][1]-q[2][1]).abs()};
  if deviation<0.25*length||level==0{out.push(q[3]);continue;}
  let midpoint=|a:[f64;2],b:[f64;2]|[(a[0]+b[0])*0.5,(a[1]+b[1])*0.5];
  let ab=midpoint(q[0],q[1]);let bc=midpoint(q[1],q[2]);let cd=midpoint(q[2],q[3]);let abc=midpoint(ab,bc);let bcd=midpoint(bc,cd);let middle=midpoint(abc,bcd);
  stack.push(([middle,bcd,cd,q[3]],level-1));stack.push(([q[0],ab,abc,middle],level-1));
 }out
}
/// Closed cubic/line contours in physical coordinates: source threshold 0.25, depth 9.
pub fn rasterize_paths(paths:&[PathContour],max_cells:usize,max_runs:usize)->Result<Vec<Span>,String>{
 if max_cells==0||max_runs==0{return Err("Gray raster admission must be positive".into());}
 let count=paths.iter().try_fold(0usize,|n,p|n.checked_add(p.segments.len()+1)).ok_or("Gray path count overflow")?;
 if count>max_cells{return Err("Gray path admission exceeded".into());}
 let mut cells=Cells{map:BTreeMap::new(),limit:max_cells,steps:0};
 for path in paths{let start=fixed(path.start)?;let mut current=start;let mut current_float=path.start;for segment in &path.segments{match segment{Segment::Line(to)=>{let next=fixed(*to)?;cells.line(current,next)?;current=next;current_float=*to;},Segment::Cubic{control1,control2,to}=>{let next=fixed(*to)?;fixed(*control1)?;fixed(*control2)?;for point in flatten_cubic(current_float,*control1,*control2,*to){let point=fixed(point)?;cells.line(current,point)?;current=point;}current=next;current_float=*to;}}}cells.line(current,start)?;}
 finish(cells,max_runs)
}
#[cfg(test)]mod tests {
 use super::*;
 #[test]fn cubic_path_admission_and_finite_controls(){
  let path=PathContour{start:[0.,0.],segments:vec![Segment::Cubic{control1:[10.,20.],control2:[20.,20.],to:[30.,0.]}]};
  assert!(!rasterize_paths(&[path.clone()],10000,10000).unwrap().is_empty());
  assert!(rasterize_paths(&[path.clone()],1,10000).is_err());
  let mut bad=path;bad.start[0]=f64::NAN;assert!(rasterize_paths(&[bad],10000,10000).is_err());
 }

 #[test]fn malformed_and_excessive_edges_refuse(){
  assert!(rasterize(&[vec![[f64::NAN,0.],[1.,0.],[1.,1.]]],100,100).is_err());
  assert!(rasterize(&[vec![[0.,0.];101]],100,100).is_err());
  assert!(rasterize(&[vec![[2_000_000.,0.],[1.,0.],[1.,1.]]],100,100).is_err());
 }
 #[test]fn subpixel_rectangle_coalesces_and_admission_rejects(){
  let p=vec![vec![[0.25,0.25],[100.,0.25],[100.,2.],[0.25,2.]]];let spans=rasterize(&p,1000,1000).unwrap();assert_eq!(spans.len(),4);assert_eq!(spans[0].coverage,144);assert_eq!(spans[1].coverage,192);
  assert!(rasterize(&p,2,1000).is_err());assert!(rasterize(&p,1000,1).is_err());
 }
}
