// Copyright (C) 2016 The Qt Company Ltd.
// SPDX-License-Identifier: GPL-3.0-only
//! Qt6.12 QRasterizer width-two selected-image independent SquareCap line.
//! Bounded source analytic fixed-point coverage, never a CPU image.
use crate::qt_gray::Span;
const F:i64=65536;
fn fp(v:f64)->i64{(v*F as f64).clamp(i32::MIN as f64*F as f64,i32::MAX as f64*F as f64)as i64}
fn mul(a:i64,b:i64)->i64{((i128::from(a)*i128::from(b))>>16)as i64}
fn div(a:f64,b:f64)->f64{if b==0.{if a>0.{1e20}else{-1e20}}else{a/b}}
fn intersect(x:i32,top:i64,bottom:i64,left:i64,right:i64,slope:i64,inverse:i64)->i64{
 let lx=i64::from(x)*F;let rx=lx+F;
 let iy=||{let l=top+mul(lx-if slope>0{left}else{right},inverse);(l,l+inverse)};
 if left>=lx&&right<=rx{mul(bottom-top,left-lx+((right-left)>>1))}
 else if left>=rx{bottom-top}
 else if left>=lx{let(_,r)=iy();(bottom-top)-mul((rx-left)>>1,if slope>0{r-top}else{bottom-r})}
 else if right<=lx{0}
 else if right<=rx{let(l,_)=iy();mul((right-lx)>>1,if slope>0{bottom-l}else{l-top})}
 else{let(l,r)=iy();if slope>0{(bottom-r)+((r-l)>>1)}else{(r-top)+((l-r)>>1)}}
}
#[derive(Clone,Copy)]struct Edge{slope:i64,inverse:i64,base:i64,base_y:i64}
impl Edge{
 fn new(start:[f64;2],end:[f64;2],base_y:i64)->Self{
  let slope=div(end[0]-start[0],end[1]-start[1]);
  Self{slope:fp(slope),inverse:fp(div(1.,slope)),base:fp(start[0]+(base_y as f64-start[1])*slope),base_y:base_y*F}
 }
 fn x(self,y:i64)->i64{self.base+mul(self.slope,y-self.base_y)}
 fn excluded(self,x:i32,top:i64,bottom:i64)->i64{
  let(a,b)=(self.x(top),self.x(bottom));
  intersect(x,top,bottom,a.min(b),a.max(b),self.slope,self.inverse)
 }
}
fn clip(a:&mut[f64;2],b:&mut[f64;2],low:[f64;2],high:[f64;2])->bool{
 for axis in 0..2{let o=1-axis;
  if a[axis]<low[axis]{if b[axis]<=low[axis]{return true;}a[o]+=(b[o]-a[o])/(b[axis]-a[axis])*(low[axis]-a[axis]);a[axis]=low[axis];}
  else if a[axis]>high[axis]{if b[axis]>=high[axis]{return true;}a[o]+=(b[o]-a[o])/(b[axis]-a[axis])*(high[axis]-a[axis]);a[axis]=high[axis];}
  if b[axis]<low[axis]{b[o]+=(b[o]-a[o])/(b[axis]-a[axis])*(low[axis]-b[axis]);b[axis]=low[axis];}
  else if b[axis]>high[axis]{b[o]+=(b[o]-a[o])/(b[axis]-a[axis])*(high[axis]-b[axis]);b[axis]=high[axis];}
 }false
}
/// This constructor implements the source line-family operator only.
/// The caller retains front clipping and independent source draw ordering.
pub fn rasterize(a:[f64;2],b:[f64;2],size:[u32;2],limit:usize)->Result<Vec<Span>,String>{
 if size.contains(&0)||size.iter().any(|v|*v>1_000_000)||limit==0{return Err("Invalid selected line admission".into());}
 if a.iter().chain(b.iter()).any(|v|!v.is_finite()){return Ok(Vec::new());}
 let mut a=a;let mut b=b;
 let point=a==b;if point{a[0]-=1.;b[0]+=1.;}
 let delta=[b[0]-a[0],b[1]-a[1]];let length=delta[0].hypot(delta[1]);let mut width=2./length;
 let cap=if point{0.}else{0.5*width};
 let mut pa=[a[0]-cap*delta[0],a[1]-cap*delta[1]];
 let mut pb=[b[0]+cap*delta[0],b[1]+cap*delta[1]];
 let off=[delta[1].abs()*width*0.5,delta[0].abs()*width*0.5];
 if clip(&mut pa,&mut pb,[-off[0],-off[1]],[size[0]as f64+off[0],size[1]as f64+off[1]]){return Ok(Vec::new());}
 let dx=pa[0]-pb[0];let dy=pa[1]-pb[1];let new_squared=dx*dx+dy*dy;if new_squared==0.{return Ok(Vec::new());}
 width*=((delta[0]*delta[0]+delta[1]*delta[1])/new_squared).sqrt();
 let near=|a:f64,b:f64|((b-a)*64.)as i64==0;
 if near(pa[1],pb[1]){
  let x=(pa[0]+pb[0])*0.5;let dx=(pb[0]-pa[0]).abs()*0.5;let y=pa[1];let dy=width*dx;
  pa=[x,y-dy];pb=[x,y+dy];width=1./width;
 }
 let mut spans=Vec::new();let mut work=0usize;
 let mut push=|x:i32,y:i32,coverage:i64|->Result<(),String>{
  if coverage==0{return Ok(());}if !(0..=255).contains(&coverage){return Err("Selected line invalid coverage".into());}
  if let Some(last)=spans.last_mut(){let last:&mut Span=last;if last.y==y&&last.coverage==coverage as u8&&i64::from(last.x)+i64::from(last.len)==i64::from(x){last.len+=1;return Ok(());}}
  if spans.len()>=limit{return Err("Selected line span admission exceeded".into());}spans.push(Span{x,y,len:1,coverage:coverage as u8});Ok(())
 };
 if near(pa[0],pb[0]){
  if pa[1]>pb[1]{std::mem::swap(&mut pa,&mut pb);}
  let half=0.5*width*(pb[1]-pa[1]);
  let left=fp((pa[0]-half).clamp(0.,size[0]as f64));let right=fp((pa[0]+half).clamp(0.,size[0]as f64));
  let top=fp(pa[1].clamp(0.,size[1]as f64));let bottom=fp(pb[1].clamp(0.,size[1]as f64));
  for y in (top/F)..=(bottom/F).min(size[1]as i64-1){for x in (left/F)..=(right/F).min(size[0]as i64-1){
   work+=1;if work>limit.saturating_mul(16){return Err("Selected line work admission exceeded".into());}
   let h=(bottom.min((y+1)*F)-top.max(y*F)).max(0);let w=(right.min((x+1)*F)-left.max(x*F)).max(0);
   push(x as i32,y as i32,mul(h,w*255)>>16)?;
  }}return Ok(spans);
 }
 if pa[1]>pb[1]{std::mem::swap(&mut pa,&mut pb);}
 let delta=[(pb[0]-pa[0])*0.5*width,(pb[1]-pa[1])*0.5*width];let perp=[delta[1],-delta[0]];
 let add=|p:[f64;2],sign:f64|[(p[0]+sign*perp[0])*64.,(p[1]+sign*perp[1])*64.].map(|v|v.floor()/64.);
 let(top,left,right,bottom)=if pa[0]<pb[0]{(add(pa,1.),add(pa,-1.),add(pb,1.),add(pb,-1.))}else{(add(pa,-1.),add(pb,-1.),add(pa,1.),add(pb,1.))};
 let start=top[1].clamp(0.,size[1]as f64-1.)as i64;
 let end=bottom[1].clamp(0.,size[1]as f64-1.)as i64;
 let tl=Edge::new(top,left,start);let tr=Edge::new(top,right,start);
 let bl=Edge::new(left,bottom,(left[1]as i64).max(start));let br=Edge::new(right,bottom,(right[1]as i64).max(start));
 let yt=fp(top[1]);let yl=fp(left[1]);let yr=fp(right[1]);let yb=fp(bottom[1]);
 for y in start..=end{
  let low=(y*F).max(yt);let high=((y+1)*F).min(yb);if high<=low{continue;}
  let lsplit=yl.clamp(low,high);let rsplit=yr.clamp(low,high);
  let edges=[(tl,low,lsplit),(bl,lsplit,high),(tr,low,rsplit),(br,rsplit,high)];
  let(mut min_x,mut max_x)=(i64::MAX,i64::MIN);
  for(edge,a,b)in edges{if b>a{for x in [edge.x(a),edge.x(b)]{min_x=min_x.min(x);max_x=max_x.max(x);}}}
  let xmin=(min_x>>16).clamp(0,size[0]as i64-1);
  let xmax=(max_x>>16).clamp(0,size[0]as i64-1);
  for x in xmin..=xmax{
   work+=1;if work>limit.saturating_mul(16){return Err("Selected line work admission exceeded".into());}
   let mut excluded=0;
   for(i,(edge,a,b))in edges.iter().enumerate(){if b>a{let v=edge.excluded(x as i32,*a,*b);excluded+=if i<2{v}else{(b-a)-v};}}
   push(x as i32,y as i32,((high-low-excluded)*255)>>16)?;
  }
 }Ok(spans)
}
