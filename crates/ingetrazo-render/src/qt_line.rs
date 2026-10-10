// Copyright (C) 2016 The Qt Company Ltd.
// SPDX-License-Identifier: GPL-3.0-only
//! Bounded Rust adaptation of Qt6.12 QRasterizer::rasterizeLine AA branch.
//! Distinct from gray path fill: 16.16 pixel-area coverage is multiplied by255.
use crate::qt_gray::Span;
const F:i64=65536;
fn fixed(x:f64)->i64{(x*F as f64).clamp(i32::MIN as f64*F as f64,i32::MAX as f64*F as f64)as i64}
fn mul(x:i64,y:i64)->i64{((i128::from(x)*i128::from(y))>>16)as i64}
fn divide(x:f64,y:f64)->f64{if y==0.{if x>0.{1e20}else{-1e20}}else{x/y}}
fn near(a:f64,b:f64)->bool{((b-a)*64.)as i64==0}
fn intersect(x:i64,top:i64,bottom:i64,left:i64,right:i64,slope:i64,inverse:i64)->i64{
 let l=x*F;let r=l+F;
 let ys=||{let y=top+mul(l-if slope>0{left}else{right},inverse);(y,y+inverse)};
 if left>=l&&right<=r{mul(bottom-top,left-l+((right-left)>>1))}
 else if left>=r{bottom-top}
 else if left>=l{let(_,yr)=ys();if slope>0{bottom-top-mul((r-left)>>1,yr-top)}else{bottom-top-mul((r-left)>>1,bottom-yr)}}
 else if right<=l{0}
 else if right<=r{let(yl,_)=ys();if slope>0{mul((right-l)>>1,bottom-yl)}else{mul((right-l)>>1,yl-top)}}
 else{let(yl,yr)=ys();if slope>0{bottom-yr+((yr-yl)>>1)}else{yr-top+((yl-yr)>>1)}}
}
struct Output{spans:Vec<Span>,limit:usize,work:usize,max_work:usize}
impl Output{
 fn add(&mut self,x:i64,len:i64,y:i64,coverage:i64)->Result<(),String>{
  self.work=self.work.checked_add(1).ok_or("Line raster work overflow")?;if self.work>self.max_work{return Err("Line raster work admission exceeded".into());}
  if len<=0||coverage==0{return Ok(());}if !(0..=255).contains(&coverage){return Err("Line raster coverage outside source domain".into());}
  let span=Span{x:i32::try_from(x).map_err(|_|"Line x overflow")?,y:i32::try_from(y).map_err(|_|"Line y overflow")?,len:u32::try_from(len).map_err(|_|"Line run overflow")?,coverage:coverage as u8};
  if let Some(last)=self.spans.last_mut(){if last.y==span.y&&last.coverage==span.coverage&&i64::from(last.x)+i64::from(last.len)==x{last.len=last.len.checked_add(span.len).ok_or("Line span length overflow")?;return Ok(());}}
  if self.spans.len()>=self.limit{return Err("Line raster span admission exceeded".into());}self.spans.try_reserve(1).map_err(|_|"Line span allocation failed")?;self.spans.push(span);Ok(())
 }
}
/// Clip is half-open physical pixels. No pixel image or per-pixel interior mesh.
pub fn rasterize(a:[f64;2],b:[f64;2],pen_width:f64,clip:[i32;4],max_work:usize,max_runs:usize)->Result<Vec<Span>,String>{
 if a.into_iter().chain(b).chain([pen_width]).any(|v|!v.is_finite()||v.abs()>1_000_000.)||clip[0]>clip[2]||clip[1]>clip[3]{return Err("Line raster coordinate admission exceeded".into());}
 let mut out=Output{spans:Vec::new(),limit:max_runs,work:0,max_work};
 if a==b||pen_width<=0.||clip[0]==clip[2]||clip[1]==clip[3]{return Ok(out.spans);}
 let dx=b[0]-a[0];let dy=b[1]-a[1];let length=(dx*dx+dy*dy).sqrt();let mut width=pen_width/length;
 let mut pa=[a[0]-0.5*width*dx,a[1]-0.5*width*dy];let mut pb=[b[0]+0.5*width*dx,b[1]+0.5*width*dy];
 if !width.is_finite()||pa.into_iter().chain(pb).any(|v|!v.is_finite()){return Err("Line derived coordinate overflow".into());}
 let offs=[dy.abs()*width*0.5,dx.abs()*width*0.5];let bounds=[f64::from(clip[0])-offs[0],f64::from(clip[1])-offs[1],f64::from(clip[2])+offs[0],f64::from(clip[3])+offs[1]];
 for axis in 0..2{let other=1-axis;let lo=bounds[axis];let hi=bounds[axis+2];
  if pa[axis]<lo{if pb[axis]<=lo{return Ok(out.spans);}pa[other]+=(pb[other]-pa[other])/(pb[axis]-pa[axis])*(lo-pa[axis]);pa[axis]=lo;}
  else if pa[axis]>hi{if pb[axis]>=hi{return Ok(out.spans);}pa[other]+=(pb[other]-pa[other])/(pb[axis]-pa[axis])*(hi-pa[axis]);pa[axis]=hi;}
  if pb[axis]<lo{pb[other]+=(pb[other]-pa[other])/(pb[axis]-pa[axis])*(lo-pb[axis]);pb[axis]=lo;}
  else if pb[axis]>hi{pb[other]+=(pb[other]-pa[other])/(pb[axis]-pa[axis])*(hi-pb[axis]);pb[axis]=hi;}
 }
 let new_length=(pa[0]-pb[0]).powi(2)+(pa[1]-pb[1]).powi(2);if new_length==0.{return Ok(out.spans);}width*=((dx*dx+dy*dy)/new_length).sqrt();
 if near(pa[1],pb[1]){let x=(pa[0]+pb[0])*0.5;let d=(pb[0]-pa[0]).abs()*0.5;let y=pa[1];let h=width*d;pa=[x,y-h];pb=[x,y+h];width=1./width;}
 if near(pa[0],pb[0]){
  if pa[1]>pb[1]{std::mem::swap(&mut pa,&mut pb);}let h=0.5*width*(pb[1]-pa[1]);
  let left=(pa[0]-h).clamp(f64::from(clip[0]),f64::from(clip[2]));let right=(pa[0]+h).clamp(f64::from(clip[0]),f64::from(clip[2]));pa[1]=pa[1].clamp(f64::from(clip[1]),f64::from(clip[3]));pb[1]=pb[1].clamp(f64::from(clip[1]),f64::from(clip[3]));
  if near(left,right)||near(pa[1],pb[1]){return Ok(out.spans);}
  let il=left as i64;let ir=right as i64;let lw=(il+1)*F-fixed(left);let rw=fixed(right)-ir*F;
  let mut groups=[(0,0,0);3];let mut n=1;
  if il==ir{groups[0]=(il,1,if lw==F{rw*255}else{(rw+lw-F)*255});}
  else{groups[0]=(il,1,lw*255);if lw==F{groups[0].1=ir-il;}else if ir-il>1{groups[1]=(il+1,ir-il-1,255*F);n+=1;}if rw!=0{groups[n]=(ir,1,rw*255);n+=1;}}
  let top=(pa[1]as i64)*F;let bottom=(pb[1]as i64)*F;let py=fixed(pa[1]);let by=fixed(pb[1]);
  for y in (top..=bottom).step_by(F as usize){if y/F>=i64::from(clip[3]){break;}let h=(y+F).min(by)-y.max(py);for &(x,len,c)in &groups[..n]{out.add(x,len,y>>16,mul(h,c)>>16)?;}}
 }else{
  if pa[1]>pb[1]{std::mem::swap(&mut pa,&mut pb);}let delta=[(pb[0]-pa[0])*0.5*width,(pb[1]-pa[1])*0.5*width];let perp=[delta[1],-delta[0]];
  let add=|p:[f64;2],sign:f64|[p[0]+perp[0]*sign,p[1]+perp[1]*sign];
  let(top,left,right,bottom)=if pa[0]<pb[0]{(add(pa,1.),add(pa,-1.),add(pb,1.),add(pb,-1.))}else{(add(pa,-1.),add(pb,-1.),add(pa,1.),add(pb,1.))};
  let snap=|p:[f64;2]|p.map(|v|(v*64.).floor()/64.);let(top,left,right,bottom)=(snap(top),snap(left),snap(right),snap(bottom));
  let tb=top[1].clamp(f64::from(clip[1]),f64::from(clip[3]-1));let bb=bottom[1].clamp(f64::from(clip[1]),f64::from(clip[3]-1));
  let slope=|a:[f64;2],b:[f64;2]|divide(b[0]-a[0],b[1]-a[1]);let tl=slope(top,left);let tr=slope(top,right);let bl=slope(left,bottom);let br=slope(right,bottom);
  let(tlf,trf,blf,brf)=(fixed(tl),fixed(tr),fixed(bl),fixed(br));let(itl,itr,ibl,ibr)=(fixed(divide(1.,tl)),fixed(divide(1.,tr)),fixed(divide(1.,bl)),fixed(divide(1.,br)));
  let(it,il,ir,ib)=((tb as i64)*F,(left[1]as i64)*F,(right[1]as i64)*F,(bb as i64)*F);
  let(mut la,mut ra)=(fixed(top[0]+(tb as i64 as f64-top[1])*tl),fixed(top[0]+(tb as i64 as f64-top[1])*tr));let(mut lb,mut rb)=(0,0);
  if il<it{lb=fixed(left[0]+(tb as i64 as f64-left[1])*bl);}if ir<it{rb=fixed(right[0]+(tb as i64 as f64-right[1])*br);}
  let(yt,yl,yr,yb)=(fixed(top[1]),fixed(left[1]),fixed(right[1]),fixed(bottom[1]));let mut row_top=it.max(yt);
  let(mut tla,mut tra)=(la+mul(tlf,row_top-it),ra+mul(trf,row_top-it));
  for y in (it..=ib).step_by(F as usize){
   let rbl=(y+F).min(yl);let rbr=(y+F).min(yr);let rtl=y.max(yl);let rtr=y.max(yr);let row_bottom=(y+F).min(yb);
   let(tlb,bla)=if y==il{lb=fixed(left[0]+((y>>16)as f64-left[1])*bl);(lb+mul(blf,rtl-y),la+mul(tlf,rbl-y))}else{(lb,la+tlf)};
   let(trb,bra)=if y==ir{rb=fixed(right[0]+((y>>16)as f64-right[1])*br);(rb+mul(brf,rtr-y),ra+mul(trf,rbr-y))}else{(rb,ra+trf)};
   let(blb,brb)=if y==ib{(lb+mul(blf,row_bottom-y),rb+mul(brf,row_bottom-y))}else{(lb+blf,rb+brf)};
   let(mut lmin,mut lmax)=if y<il{(bla>>16,tla>>16)}else if y==il{(bla.max(tlb)>>16,tla.max(blb)>>16)}else{(tlb>>16,blb>>16)};
   let(mut rmin,mut rmax)=if y<ir{(tra>>16,bra>>16)}else if y==ir{(tra.min(brb)>>16,bra.min(trb)>>16)}else{(brb>>16,trb>>16)};
   let bound=|v:i64|v.clamp(i64::from(clip[0]),i64::from(clip[2]-1));lmin=bound(lmin);lmax=bound(lmax);rmin=bound(rmin);rmax=bound(rmax);lmax=lmax.min(rmax);rmin=rmin.max(lmin);
   let height=row_bottom-row_top;let mut x=lmin;
   while x<=lmax{
    let mut excluded=0;
    if y<=il&&rbl>row_top{excluded+=intersect(x,row_top,rbl,bla,tla,tlf,itl);}
    if y>=il&&row_bottom>rtl{excluded+=intersect(x,rtl,row_bottom,tlb,blb,blf,ibl);}
    if x>=rmin{if y<=ir&&rbr>row_top{excluded+=(rbr-row_top)-intersect(x,row_top,rbr,tra,bra,trf,itr);}if y>=ir&&row_bottom>rtr{excluded+=(row_bottom-rtr)-intersect(x,rtr,row_bottom,brb,trb,brf,ibr);}}
    out.add(x,1,y>>16,(255*(height-excluded))>>16)?;x+=1;
   }
   if x<rmin{out.add(x,rmin-x,y>>16,(255*height)>>16)?;x=rmin;}
   while x<=rmax{let mut excluded=0;if y<=ir&&rbr>row_top{excluded+=(rbr-row_top)-intersect(x,row_top,rbr,tra,bra,trf,itr);}if y>=ir&&row_bottom>rtr{excluded+=(row_bottom-rtr)-intersect(x,rtr,row_bottom,brb,trb,brf,ibr);}out.add(x,1,y>>16,(255*(height-excluded))>>16)?;x+=1;}
   la+=tlf;lb+=blf;ra+=trf;rb+=brf;tla=la;tra=ra;row_top=y+F;
  }
 }
 Ok(out.spans)
}
#[cfg(test)]mod tests{use super::*;
 #[test]fn dimension_line_uses_source_255_area_not_gray_256(){let spans=rasterize([160.,100.],[280.,100.],1.5,[0,0,640,400],10000,1000).unwrap();assert_eq!(spans.iter().find(|s|s.y==99&&s.x<=200&&s.x+i32::try_from(s.len).unwrap()>200).unwrap().coverage,191);}
 #[test]fn refusal_is_bounded_before_publication(){assert!(rasterize([f64::NAN,0.],[10.,1.],1.5,[0,0,640,400],100,100).is_err());assert!(rasterize([0.,0.],[100.,100.],2.,[0,0,640,400],1,100).is_err());}
}
