// SPDX-License-Identifier: GPL-3.0-or-later
//! Source image-border front clipping and projection. No hidden-line or near-z clip:
//! original overlays clip homogeneous w only and remain on top of model surfaces.
fn clip(m:&[f32;16],p:[f32;3])->[f32;4]{
 // Qt QMatrix4x4::map(QVector4D) source left-to-right float arithmetic.
 std::array::from_fn(|r|m[r]*p[0]+m[4+r]*p[1]+m[8+r]*p[2]+m[12+r])
}
pub fn projected_segment(m:&[f32;16],mut a:[f32;3],mut b:[f32;3],size:[u32;2])->Option<[[f64;2];2]>{
 let wa=clip(m,a)[3];let wb=clip(m,b)[3];let eps=1e-3f64;
 if f64::from(wa)<=eps&&f64::from(wb)<=eps{return None;}
 if f64::from(wa)<=eps||f64::from(wb)<=eps{
  let t=((eps-f64::from(wa))/(f64::from(wb)-f64::from(wa)))as f32;
  let cut=std::array::from_fn(|i|a[i]+(b[i]-a[i])*t);
  if f64::from(wa)<=eps{a=cut;}else{b=cut;}
 }
 let project=|p|{let c=clip(m,p);if c[3]<=0.{return None;}
  let x=f64::from(c[0])/f64::from(c[3]);let y=f64::from(c[1])/f64::from(c[3]);
  Some([(x*0.5+0.5)*f64::from(size[0]),(1.-(y*0.5+0.5))*f64::from(size[1])])
 };
 Some([project(a)?,project(b)?])
}
