// Copyright (C) 2016 The Qt Company Ltd.
// SPDX-License-Identifier: GPL-3.0-only
//! Bounded Qt6.12 QCosmeticStroker antialiased solid pen path mathematics.
//! Ordered segments preserve source repeated-pixel SourceOver; no pixel image.
use std::collections::BTreeMap;
use crate::qt_gray::{PathContour,Segment,Span};
// Literal QCosmeticStroker::clipLine operation order. The extra pixel bounds
// are deviceRect.left/top-1 and right/bottom+2 (width/height+1).
fn clip_line(a:&mut [f64;2],b:&mut [f64;2],size:[u32;2])->bool{
 if size.contains(&0)||a.iter().chain(b.iter()).any(|v|!v.is_finite()){return false;}
 let min=[-1.;2];let max=[f64::from(size[0])+1.,f64::from(size[1])+1.];
 if a[0]<min[0]{if b[0]<=min[0]{return false;}a[1]+=(b[1]-a[1])/(b[0]-a[0])*(min[0]-a[0]);a[0]=min[0];}else if a[0]>max[0]{if b[0]>=max[0]{return false;}a[1]+=(b[1]-a[1])/(b[0]-a[0])*(max[0]-a[0]);a[0]=max[0];}
 if b[0]<min[0]{b[1]+=(b[1]-a[1])/(b[0]-a[0])*(min[0]-b[0]);b[0]=min[0];}else if b[0]>max[0]{b[1]+=(b[1]-a[1])/(b[0]-a[0])*(max[0]-b[0]);b[0]=max[0];}
 if a[1]<min[1]{if b[1]<=min[1]{return false;}a[0]+=(b[0]-a[0])/(b[1]-a[1])*(min[1]-a[1]);a[1]=min[1];}else if a[1]>max[1]{if b[1]>=max[1]{return false;}a[0]+=(b[0]-a[0])/(b[1]-a[1])*(max[1]-a[1]);a[1]=max[1];}
 if b[1]<min[1]{b[0]+=(b[0]-a[0])/(b[1]-a[1])*(min[1]-b[1]);b[1]=min[1];}else if b[1]>max[1]{b[0]+=(b[0]-a[0])/(b[1]-a[1])*(max[1]-b[1]);b[1]=max[1];}true
}
struct Writer{spans:Vec<Span>,limit:usize,work:usize,opacity:i64,dash:bool,pattern_offset:i64,custom_pattern:Option<[Vec<i64>;2]>,device:Option<[u32;2]>}
impl Writer{
 fn line(&mut self,a:[f64;2],b:[f64;2],mut begin:bool,mut end:bool)->Result<(),String>{
  let(mut a,mut b)=(a,b);if let Some(size)=self.device{if !clip_line(&mut a,&mut b,size){return Ok(());}}
  if a.iter().chain(b.iter()).any(|x|!x.is_finite()||x.abs()>1_000_000.){return Err("Cosmetic input outside finite bounded coordinates".into());}
  let fixed=|x:f64|(x*64.)as i64;let(mut x1,mut y1,mut x2,mut y2)=(fixed(a[0]),fixed(a[1]),fixed(b[0]),fixed(b[1]));
  // Qt selects the major axis after F26Dot6 conversion, including fixed-grid ties.
  let vertical=(x2-x1).abs()<(y2-y1).abs();if vertical{std::mem::swap(&mut x1,&mut y1);std::mem::swap(&mut x2,&mut y2);}
  if x1==x2{return Ok(());}let inc=((y2-y1)*65536)/(x2-x1);
  let reversed=x1>x2;
  if reversed{std::mem::swap(&mut x1,&mut x2);std::mem::swap(&mut y1,&mut y2);std::mem::swap(&mut begin,&mut end);}
  let mut minor=(y1-32)*1024-((((x1&63)-32)*inc)>>6);if begin{x1-=32;minor-=inc>>1;}if end{x2+=32;}
  // Qt DashLine [4,2] in F26Dot6. Each independent drawLine resets
  // patternOffset, and reversed major-axis traversal uses [2,4].
  let default_pattern=if reversed{[128i64,384]}else{[256i64,384]};
  let pattern=self.custom_pattern.as_ref().map_or(&default_pattern[..],|p|p[usize::from(reversed)].as_slice());let pattern_length=*pattern.last().unwrap();
  let mut dash_offset=if reversed{pattern_length-self.pattern_offset-(x2-x1)-((x1&63)-32)}else{self.pattern_offset-((x1&63)-32)}.rem_euclid(pattern_length);
  if self.dash{self.pattern_offset=(self.pattern_offset+x2-x1).rem_euclid(pattern_length);}
  let mut dash_index=0;while dash_index+1<pattern.len()&&dash_offset>=pattern[dash_index]{dash_index+=1;}
  let start=x1>>6;let finish=x2>>6;let mut pixels=BTreeMap::new();
  for major in start..=finish{
   self.work+=1;if self.work>self.limit.saturating_mul(8){return Err("Cosmetic edge work admission exceeded".into());}
   let covered=if start==finish{x2-x1}else if major==start{64-(x1&63)}else if major==finish{x2&63}else{64};
   let base=minor>>16;let fraction=(minor>>8)&255;
   let dash_on=!self.dash||((dash_index+usize::from(!reversed))&1)!=0;
   for(row,cov)in[(base,255-fraction),(base+1,fraction)]{if !dash_on{continue;}let coverage=(((cov*covered)>>6)*self.opacity>>8)as u8;if coverage!=0{let(x,y)=if vertical{(row,major)}else{(major,row)};if pixels.len()>=self.limit.saturating_mul(16)&&!pixels.contains_key(&(y,x)){return Err("Cosmetic temporary pixel admission exceeded".into());}pixels.insert((y,x),coverage);}}
   dash_offset+=64;if dash_offset>=pattern[dash_index]{dash_index=(dash_index+1)%pattern.len();}dash_offset%=pattern_length;
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
 rasterize_path_with_caps(path,closed,width,true,limit)
}
/// Literal QCosmeticStroker::drawCaps = capStyle != Qt::FlatCap. RoundCap
/// and SquareCap share its half-pixel endpoint extension; FlatCap omits it.
pub fn rasterize_path_with_caps(path:&PathContour,closed:bool,width:f64,draw_caps:bool,limit:usize)->Result<Vec<Span>,String>{
 rasterize_path_device(path,closed,width,draw_caps,None,limit)
}
/// Qt clips to the original physical image before F26Dot6; tiling must not
/// substitute tile bounds or change the interpolation/phase of cropped lines.
pub fn rasterize_path_device(path:&PathContour,closed:bool,width:f64,draw_caps:bool,device:Option<[u32;2]>,limit:usize)->Result<Vec<Span>,String>{
 if !width.is_finite()||!(0. ..=1.).contains(&width)||limit==0||path.segments.len()>limit{return Err("Invalid cosmetic pen admission".into());}
 let opacity=if width==0.{256}else{(width*256.)as i64};let mut writer=Writer{spans:Vec::new(),limit,work:0,opacity,dash:false,pattern_offset:0,custom_pattern:None,device};let mut current=path.start;
 for(i,segment)in path.segments.iter().enumerate(){let begin=draw_caps&&!closed&&i==0;let end=draw_caps&&!closed&&i+1==path.segments.len();match segment{Segment::Line(to)=>{writer.line(current,*to,begin,end)?;current=*to;},Segment::Cubic{control1,control2,to}=>{writer.cubic([current,*control1,*control2,*to],6,begin,end)?;current=*to;}}}
 Ok(writer.spans)
}
/// Source custom cosmetic pen pattern (in QPen width units, intentionally
/// not scaled by the device transform). Each independent drawLine resets it.
pub fn rasterize_custom_line(a:[f64;2],b:[f64;2],width:f64,draw_caps:bool,pen_pattern:&[f64],limit:usize)->Result<Vec<Span>,String>{
 rasterize_custom_line_device(a,b,width,draw_caps,pen_pattern,None,limit)
}
pub fn rasterize_custom_line_device(a:[f64;2],b:[f64;2],width:f64,draw_caps:bool,pen_pattern:&[f64],device:Option<[u32;2]>,limit:usize)->Result<Vec<Span>,String>{
 if !width.is_finite()||!(0. ..=1.).contains(&width)||limit==0||pen_pattern.is_empty()||pen_pattern.len()>1024||pen_pattern.len()%2!=0||pen_pattern.iter().any(|v|!v.is_finite()||*v<=0.){return Err("Invalid source custom cosmetic pattern".into());}
 let cumulative=|reverse:bool|{let mut total=0i64;(0..pen_pattern.len()).map(|i|{let i=if reverse{pen_pattern.len()-1-i}else{i};total+=((pen_pattern[i]*64.)as i64).clamp(1,65536);total}).collect::<Vec<_>>()};
 let mut writer=Writer{spans:Vec::new(),limit,work:0,opacity:if width==0.{256}else{(width*256.)as i64},dash:true,pattern_offset:0,custom_pattern:Some([cumulative(false),cumulative(true)]),device};
 writer.line(a,b,draw_caps,draw_caps)?;Ok(writer.spans)
}
/// Continuous source cosmetic DashLine path; device [4,2] pattern carries across
/// segments and source cubic subdivisions. Existing independent-line API resets.
pub fn rasterize_dashed_path(path:&PathContour,closed:bool,width:f64,limit:usize)->Result<Vec<Span>,String>{
 if !width.is_finite()||!(0. ..=1.).contains(&width)||limit==0||path.segments.len()>limit{return Err("Invalid cosmetic dashed path admission".into());}
 let opacity=if width==0.{256}else{(width*256.)as i64};let mut writer=Writer{spans:Vec::new(),limit,work:0,opacity,dash:true,pattern_offset:0,custom_pattern:None,device:None};let mut current=path.start;
 for(i,s)in path.segments.iter().enumerate(){let begin=!closed&&i==0;let end=!closed&&i+1==path.segments.len();match s{Segment::Line(to)=>{writer.line(current,*to,begin,end)?;current=*to;},Segment::Cubic{control1,control2,to}=>{writer.cubic([current,*control1,*control2,*to],6,begin,end)?;current=*to;}}}
 Ok(writer.spans)
}
/// Source Qt6.12 independent antialiased width-one DashLine, [4,2].
/// Input must already have source device/front clipping applied. Per-edge
/// restart is intentional; this does not turn a closed path into one dash run.
pub fn rasterize_dashed_line(a:[f64;2],b:[f64;2],limit:usize)->Result<Vec<Span>,String>{
 if limit==0{return Err("Invalid cosmetic dash admission".into());}
 let mut writer=Writer{spans:Vec::new(),limit,work:0,opacity:256,dash:true,pattern_offset:0,custom_pattern:None,device:None};
 writer.line(a,b,true,true)?;Ok(writer.spans)
}
/// Qt rough floating device clip before fixed-point dash initialization,
/// followed by the exact pixel clip. Expanded bounds match deviceRect +/-1/2.
pub fn rasterize_dashed_line_clipped(mut a:[f64;2],mut b:[f64;2],size:[u32;2],limit:usize)->Result<Vec<Span>,String>{
 if size.contains(&0)||size.iter().any(|v|*v>1_000_000)||limit==0{return Err("Invalid dash device admission".into());}
 if a.iter().chain(b.iter()).any(|v|!v.is_finite()){return Ok(Vec::new());}
 for axis in 0..2{
  let other=1-axis;let low=-1.;let high=f64::from(size[axis])+1.;
  if a[axis]<low{
   if b[axis]<=low{return Ok(Vec::new());}
   a[other]+=(b[other]-a[other])/(b[axis]-a[axis])*(low-a[axis]);a[axis]=low;
  }else if a[axis]>high{
   if b[axis]>=high{return Ok(Vec::new());}
   a[other]+=(b[other]-a[other])/(b[axis]-a[axis])*(high-a[axis]);a[axis]=high;
  }
  if b[axis]<low{b[other]+=(b[other]-a[other])/(b[axis]-a[axis])*(low-b[axis]);b[axis]=low;}
  else if b[axis]>high{b[other]+=(b[other]-a[other])/(b[axis]-a[axis])*(high-b[axis]);b[axis]=high;}
 }
 let mut spans=rasterize_dashed_line(a,b,limit)?;
 spans.retain_mut(|span|{
  if span.y<0||i64::from(span.y)>=i64::from(size[1]){return false;}
  let left=i64::from(span.x).max(0);let right=(i64::from(span.x)+i64::from(span.len)).min(i64::from(size[0]));
  if right<=left{return false;}span.x=left as i32;span.len=(right-left)as u32;true
 });
 Ok(spans)
}
#[cfg(test)]mod tests{use super::*;
 #[test]fn thin_line_coalesces_and_refuses_work(){let path=PathContour{start:[10.,10.],segments:vec![Segment::Line([100.,10.])]};let spans=rasterize_path(&path,false,0.7,1000).unwrap();assert!(spans.len()<10);assert!(rasterize_path(&path,false,0.7,1).is_err());assert!(rasterize_path(&path,false,1.1,1000).is_err());}
}

/// Engineering metadata only: source-ordered maximal disjoint prefixes.
/// Integer one-row spans from the source kernel may overlap across segments.
/// A barrier between these ranges suffices for pixel-aligned span draws;
/// arbitrary transformed triangles do not satisfy this contract.
pub fn disjoint_ranges(spans:&[Span],limit:usize)->Result<Vec<std::ops::Range<usize>>,String>{
 if spans.len()>limit{return Err("Cosmetic ordering admission exceeded".into());}
 let mut occupied=BTreeMap::<(i32,i32),i64>::new();let mut ranges=Vec::new();let mut first=0;
 for(i,s)in spans.iter().enumerate(){
  let end=i64::from(s.x)+i64::from(s.len);if s.len==0||end>i64::from(i32::MAX){return Err("Invalid cosmetic ordering span".into());}
  let prior=occupied.range((s.y,i32::MIN)..=(s.y,s.x)).next_back().is_some_and(|(_,e)|*e>i64::from(s.x));
  let next=occupied.range((s.y,s.x)..=(s.y,i32::MAX)).next().is_some_and(|((_,x),_)|i64::from(*x)<end);
  if prior||next{ranges.push(first..i);first=i;occupied.clear();}
  occupied.insert((s.y,s.x),end);
 }
 if first<spans.len(){ranges.push(first..spans.len());}Ok(ranges)
}
#[cfg(test)]mod ordering_tests{use super::*;
 #[test]fn only_overlap_flushes_and_order_is_unchanged(){let spans=vec![Span{x:0,y:0,len:4,coverage:100},Span{x:4,y:0,len:8,coverage:200},Span{x:0,y:1,len:12,coverage:150},Span{x:3,y:0,len:2,coverage:70},Span{x:20,y:0,len:4,coverage:170}];assert_eq!(disjoint_ranges(&spans,5).unwrap(),vec![0..3,3..5]);assert!(disjoint_ranges(&spans,4).is_err());}
 #[test]fn disjoint_prefixes_cover_all_source_spans(){let path=PathContour{start:[0.,0.],segments:vec![Segment::Line([10.,0.]),Segment::Line([0.,0.])]};let spans=rasterize_path(&path,false,0.7,1000).unwrap();let ranges=disjoint_ranges(&spans,1000).unwrap();assert!(ranges.len()>1);assert!(ranges.len()<spans.len());assert_eq!(ranges.iter().flat_map(|r|r.clone()).collect::<Vec<_>>(),(0..spans.len()).collect::<Vec<_>>());for range in ranges{for a in range.clone(){for b in a+1..range.end{let(a,b)=(&spans[a],&spans[b]);assert!(a.y!=b.y||i64::from(a.x)+i64::from(a.len)<=i64::from(b.x)||i64::from(b.x)+i64::from(b.len)<=i64::from(a.x));}}}}
}
