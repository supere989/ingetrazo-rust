// Copyright (C) 2016 The Qt Company Ltd.
// SPDX-License-Identifier: GPL-3.0-only
//! Bounded Qt6.12 QCosmeticStroker antialiased solid pen path mathematics.
//! Ordered segments preserve source repeated-pixel SourceOver; no pixel image.
use std::collections::BTreeMap;
use crate::qt_gray::{PathContour,Segment,Span};
struct Writer{spans:Vec<Span>,limit:usize,work:usize,opacity:i64}
impl Writer{
 fn line(&mut self,mut a:[f64;2],mut b:[f64;2],mut begin:bool,mut end:bool)->Result<(),String>{
  if a.iter().chain(b.iter()).any(|x|!x.is_finite()||x.abs()>1_000_000.){return Err("Cosmetic input outside finite bounded coordinates".into());}
  let vertical=(b[0]-a[0]).abs()<(b[1]-a[1]).abs();if vertical{a.swap(0,1);b.swap(0,1);}
  let fixed=|x:f64|(x*64.)as i64;let(mut x1,mut y1,mut x2,mut y2)=(fixed(a[0]),fixed(a[1]),fixed(b[0]),fixed(b[1]));
  if x1==x2{return Ok(());}let inc=((y2-y1)*65536)/(x2-x1);
  if x1>x2{std::mem::swap(&mut x1,&mut x2);std::mem::swap(&mut y1,&mut y2);std::mem::swap(&mut begin,&mut end);}
  let mut minor=(y1-32)*1024-((((x1&63)-32)*inc)>>6);if begin{x1-=32;minor-=inc>>1;}if end{x2+=32;}
  let start=x1>>6;let finish=x2>>6;let mut pixels=BTreeMap::new();
  for major in start..=finish{
   self.work+=1;if self.work>self.limit.saturating_mul(8){return Err("Cosmetic edge work admission exceeded".into());}
   let covered=if start==finish{x2-x1}else if major==start{64-(x1&63)}else if major==finish{x2&63}else{64};
   let base=minor>>16;let fraction=(minor>>8)&255;
   for(row,cov)in[(base,255-fraction),(base+1,fraction)]{let coverage=(((cov*covered)>>6)*self.opacity>>8)as u8;if coverage!=0{let(x,y)=if vertical{(row,major)}else{(major,row)};if pixels.len()>=self.limit.saturating_mul(16)&&!pixels.contains_key(&(y,x)){return Err("Cosmetic temporary pixel admission exceeded".into());}pixels.insert((y,x),coverage);}}
   minor+=inc;
  }
  // Sorting unique pixels within one segment does not reorder any overlap.
  // Different source segments remain distinct and ordered for SourceOver.
  let mut local:Vec<Span>=Vec::new();for((y,x),coverage)in pixels{let x=i32::try_from(x).map_err(|_|"Cosmetic coordinate overflow")?;let y=i32::try_from(y).map_err(|_|"Cosmetic coordinate overflow")?;
   if let Some(last)=local.last_mut(){if last.y==y&&last.coverage==coverage&&i64::from(last.x)+i64::from(last.len)==i64::from(x){last.len+=1;continue;}}
   if self.spans.len()+local.len()>=self.limit{return Err("Cosmetic span admission exceeded".into());}local.push(Span{x,y,len:1,coverage});
  }self.spans.extend(local);Ok(())
 }
 fn cubic(&mut self,q:[[f64;2];4],level:u8,begin:bool,end:bool)->Result<(),String>{
  let dx=q[3][0]-q[0][0];let dy=q[3][1]-q[0][1];let length=0.25*(dx.abs()+dy.abs());
  if level>0&&[q[1],q[2]].iter().any(|p|(dx*(q[3][1]-p[1])-dy*(q[3][0]-p[0])).abs()>=length){
   let mid=|a:[f64;2],b:[f64;2]|[(a[0]+b[0])*0.5,(a[1]+b[1])*0.5];let ab=mid(q[0],q[1]);let bc=mid(q[1],q[2]);let cd=mid(q[2],q[3]);let abc=mid(ab,bc);let bcd=mid(bc,cd);let m=mid(abc,bcd);
   self.cubic([q[0],ab,abc,m],level-1,begin,false)?;self.cubic([m,bcd,cd,q[3]],level-1,false,end)
  }else{self.line(q[0],q[3],begin,end)}
 }
}
/// Solid antialiased pen, physical points and width <=1; no dash or clipLine policy.
pub fn rasterize_path(path:&PathContour,closed:bool,width:f64,limit:usize)->Result<Vec<Span>,String>{
 if !width.is_finite()||!(0. ..=1.).contains(&width)||limit==0||path.segments.len()>limit{return Err("Invalid cosmetic pen admission".into());}
 let opacity=if width==0.{256}else{(width*256.)as i64};let mut writer=Writer{spans:Vec::new(),limit,work:0,opacity};let mut current=path.start;
 for(i,segment)in path.segments.iter().enumerate(){let begin=!closed&&i==0;let end=!closed&&i+1==path.segments.len();match segment{Segment::Line(to)=>{writer.line(current,*to,begin,end)?;current=*to;},Segment::Cubic{control1,control2,to}=>{writer.cubic([current,*control1,*control2,*to],6,begin,end)?;current=*to;}}}
 Ok(writer.spans)
}
#[cfg(test)]mod tests{use super::*;
 #[test]fn thin_line_coalesces_and_refuses_work(){let path=PathContour{start:[10.,10.],segments:vec![Segment::Line([100.,10.])]};let spans=rasterize_path(&path,false,0.7,1000).unwrap();assert!(spans.len()<10);assert!(rasterize_path(&path,false,0.7,1).is_err());assert!(rasterize_path(&path,false,1.1,1000).is_err());}
}
