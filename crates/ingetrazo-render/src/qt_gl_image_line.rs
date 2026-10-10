// Copyright (C) 2016 The Qt Company Ltd.
// SPDX-License-Identifier: GPL-3.0-only
//! Operative original QOpenGL2PaintEngine QDashedStrokeProcessor and
//! QTriangulatingStroker independent straight line. Native GPU rasterizes strips;
//! no CPU coverage image or scan converter. Opacity-one source pens only.
pub type Strip=[[f32;2];8];
fn strip(a:[f64;2],b:[f64;2],width:f32)->Option<Strip>{
 let a=a.map(|v|v as f32);let b=b.map(|v|v as f32);
 if a==b{return None;}
 let dx=b[0]-a[0];let dy=b[1]-a[1];let pw=(width/2.)/dx.hypot(dy);
 let nx=-dy*pw;let ny=dx*pw;
 let begin=[a[0]-ny,a[1]+nx];let end=[b[0]+ny,b[1]-nx];
 let pair=|p:[f32;2]|[[p[0]+nx,p[1]+ny],[p[0]-nx,p[1]-ny]];
 let [p0,p1]=pair(begin);let[p2,p3]=pair(a);let[p4,p5]=pair(b);let[p6,p7]=pair(end);
 Some([p0,p1,p2,p3,p4,p5,p6,p7])
}
/// Per-edge dash phase resets. Default QPen SquareCap extends each4px dash,
/// creating source-visible5-on/1-off horizontal pixel runs with a1px pen.
/// Source dash repetitionLimit10000 switches exceptionally long lines to solid.
pub fn strips(a:[f64;2],b:[f64;2],selected:bool,size:[u32;2],limit:usize)->Result<Vec<Strip>,String>{
 if size.contains(&0)||limit==0{return Err("Invalid GL image-line admission".into());}
 if a.iter().chain(b.iter()).any(|v|!v.is_finite()){return Ok(Vec::new());}
 let width=if selected{2.}else{1.};let padding=8.;
 if a[0].max(b[0])<=-padding||a[0].min(b[0])>=f64::from(size[0])+padding||a[1].max(b[1])<=-padding||a[1].min(b[1])>=f64::from(size[1])+padding{return Ok(Vec::new());}
 let dx=b[0]-a[0];let dy=b[1]-a[1];let length=dx.hypot(dy);
 if selected||length/6.>10_000.{return Ok(strip(a,b,width).into_iter().collect());}
 let mut out=Vec::new();let mut pos=0.;let mut index=0;
 let at=|t:f64|if t>=length{b}else{[a[0]+dx*(t/length),a[1]+dy*(t/length)]};
 while pos<length{
  let end=(pos+if index==0{4.}else{2.}).min(length);
  if index==0{
   let p=at(pos);let q=at(end);
   if p[0].max(q[0])>-padding&&p[0].min(q[0])<f64::from(size[0])+padding&&p[1].max(q[1])>-padding&&p[1].min(q[1])<f64::from(size[1])+padding{
    if out.len()>=limit{return Err("GL image-line strip admission exceeded".into());}
    if let Some(s)=strip(p,q,width){out.push(s);}
   }
  }
  if end<=pos{return Err("GL image-line dash progress failure".into());}pos=end;index^=1;
 }
 Ok(out)
}
