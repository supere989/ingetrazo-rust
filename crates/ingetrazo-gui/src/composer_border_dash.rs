// Copyright (C) 2016 The Qt Company Ltd.
// SPDX-License-Identifier: GPL-3.0-only
//! Bounded Qt6.12 flattened dash traversal and literal rounded polyline stroke.
use ingetrazo_render::qt_gray::{PathContour,Segment};
type Point=[f64;2];type Cubic=[Point;4];const K:f64=0.5522847498;
fn mix(a:Point,b:Point,t:f64)->Point{[a[0]+t*(b[0]-a[0]),a[1]+t*(b[1]-a[1])]}
fn split(q:Cubic,t:f64)->(Cubic,Cubic){let ab=mix(q[0],q[1],t);let bc=mix(q[1],q[2],t);let cd=mix(q[2],q[3],t);let abc=mix(ab,bc,t);let bcd=mix(bc,cd,t);let m=mix(abc,bcd,t);([q[0],ab,abc,m],[m,bcd,cd,q[3]])}
fn half(q:Cubic)->(Cubic,Cubic){let mid=|a:Point,b:Point|[(a[0]+b[0])*0.5,(a[1]+b[1])*0.5];let ab=mid(q[0],q[1]);let bc=mid(q[1],q[2]);let cd=mid(q[2],q[3]);let abc=mid(ab,bc);let bcd=mid(bc,cd);let m=mid(abc,bcd);([q[0],ab,abc,m],[m,bcd,cd,q[3]])}
fn interval(q:Cubic,a:f64,b:f64)->Cubic{let(_,right)=split(q,a);split(right,(b-a)/(1.-a)).0}
fn arc_t(a:f64)->f64{
 if a.abs()<=1e-12{return 0.;}if (a-90.).abs()*1e12<=a.abs().min(90.){return 1.;}
 let rad=a*(std::f64::consts::PI/180.);let(mut tc,cos,sin)=(a/90.,rad.cos(),rad.sin());
 for _ in 0..2{tc-=((((2.-3.*K)*tc+3.*(K-1.))*tc)*tc+1.-cos)/(((6.-9.*K)*tc+6.*(K-1.))*tc);}
 let mut ts=tc;for _ in 0..2{ts-=((((3.*K-2.)*ts-6.*K+3.)*ts+3.*K)*ts-sin)/(((9.*K-6.)*ts+12.*K-6.)*ts+3.*K);}
 0.5*(tc+ts)
}
fn ellipse_point(center:Point,r:f64,angle:f64)->Point{
 let theta=angle-360.*(angle/360.).floor();let mut t=theta/90.;let quadrant=t as i32;t-=f64::from(quadrant);t=arc_t(90.*t);if quadrant&1!=0{t=1.-t;}
 let mt=1.-t;let mut b=mt*mt;let mut c=t*t;let d=c*t;let a=b*mt;b*=3.*t;c*=3.*mt;let(mut x,mut y)=(a+b+c*K,d+c+b*K);if quadrant==1||quadrant==2{x=-x;}if quadrant==0||quadrant==1{y=-y;}[center[0]+r*x,center[1]+r*y]
}
fn arc(out:&mut Vec<Segment>,center:Point,r:f64,start:f64,sweep:f64)->Result<(),String>{
 if !start.is_finite()||!sweep.is_finite()||sweep>0.||sweep< -360.{return Err("Invalid source dash arc admission".into());}
 let left=center[0]-r;let top=center[1]-r;let right=left+2.*r;let bottom=top+2.*r;let x=left+r;let y=top+r;
 let points=[[right,y],[right,y+r*K],[x+r*K,bottom],[x,bottom],[x-r*K,bottom],[left,y+r*K],[left,y],[left,y-r*K],[x-r*K,top],[x,top],[x+r*K,top],[right,y-r*K],[right,y]];
 let mut first=(start/90.).floor()as i32;let mut last=((start+sweep)/90.).floor()as i32;let mut a=1.-(start-f64::from(first)*90.)/90.;let mut b=1.-(start+sweep-f64::from(last)*90.)/90.;
 if (a-1.).abs()<=1e-12{a=0.;first-=1;}if b.abs()<=1e-12{b=1.;last+=1;}
 a=arc_t(a*90.);b=arc_t(b*90.);
 if first==last-1{return Ok(());}
 let end=ellipse_point([x,y],r,start+sweep);let mut i=first;let mut emitted=0;
 while i!=last-1{if emitted>=4{return Err("Source dash arc count admission exceeded".into());}let quadrant=3-i.rem_euclid(4)as usize;let j=3*quadrant;let mut q=[points[j],points[j+1],points[j+2],points[j+3]];
  if first==last&&(a-b).abs()*1e12<=a.abs().min(b.abs()){return Ok(());}
  if i==first{if i==last&&(b-1.).abs()>1e-12{q=interval(q,a,b);}else if a.abs()>1e-12{q=interval(q,a,1.);}}
  else if i==last&&(b-1.).abs()>1e-12{q=interval(q,0.,b);}
  if i==last{q[3]=end;}out.push(Segment::Cubic{control1:q[1],control2:q[2],to:q[3]});emitted+=1;i-=1;
 }Ok(())
}
fn equal(a:f64,b:f64)->bool{if a==0.||b==0.{(a-b).abs()<=1e-12}else{(a-b).abs()*1e12<=a.abs().min(b.abs())}}
fn direction(a:Point,b:Point)->Result<(Point,f64,f64),String>{let d=[b[0]-a[0],b[1]-a[1]];let len=d[0].hypot(d[1]);if !len.is_finite()||len<=0.{return Err("Invalid source dash tangent".into());}let angle=(-d[1]).atan2(d[0])*(180./std::f64::consts::PI);let angle=if angle<0.{angle+360.}else{angle};let angle=if (angle-360.).abs()*1e12<=angle.abs().min(360.){0.}else{angle};Ok(([d[0]/len,d[1]/len],len,angle))}
fn stroke(points:&[Point],r:f64)->Result<PathContour,String>{
 let mut p=Vec::new();for point in points{if p.last().is_none_or(|a:&Point|!equal(a[0],point[0])||!equal(a[1],point[1])){p.push(*point);}}
 if p.len()<2{return Ok(PathContour{start:p.first().copied().unwrap_or([0.;2]),segments:Vec::new()});}
 let side=|p:&[Point],out:&mut Vec<Segment>|->Result<(),String>{for i in 0..p.len()-1{let(u,_,angle)=direction(p[i],p[i+1])?;out.push(Segment::Line([p[i+1][0]+u[1]*r,p[i+1][1]-u[0]*r]));if i+2<p.len(){let(v,len,next)=direction(p[i+1],p[i+2])?;let n=[p[i+1][0]+v[1]*r,p[i+1][1]-v[0]*r];let cross=u[0]*v[1]-u[1]*v[0];if cross<0.&&len>r{out.push(Segment::Line(p[i+1]));}else if cross!=0.{arc(out,p[i+1],r,angle+90.,-(next-angle).abs())?;}out.push(Segment::Line(n));}}Ok(())};
 let(first,_,_)=direction(p[0],p[1])?;let start=[p[0][0]+first[1]*r,p[0][1]-first[0]*r];let mut out=Vec::new();side(&p,&mut out)?;let last=p[p.len()-1];let(end,_,_)=direction(p[p.len()-2],last)?;
 out.extend([Segment::Line([last[0]+(end[1]+end[0])*r,last[1]+(-end[0]+end[1])*r]),Segment::Line([last[0]+(-end[1]+end[0])*r,last[1]+(end[0]+end[1])*r]),Segment::Line([last[0]-end[1]*r,last[1]+end[0]*r])]);
 let reverse=p.iter().rev().copied().collect::<Vec<_>>();side(&reverse,&mut out)?;let a=p[0];out.extend([Segment::Line([a[0]+(-first[1]-first[0])*r,a[1]+(first[0]-first[1])*r]),Segment::Line([a[0]+(first[1]-first[0])*r,a[1]+(-first[0]-first[1])*r]),Segment::Line(start)]);Ok(PathContour{start,segments:out})
}
fn flatten(q:Cubic,threshold:f64,level:u8,out:&mut Vec<Point>)->Result<(),String>{
 let dx=q[3][0]-q[0][0];let dy=q[3][1]-q[0][1];let mut len=dx.abs()+dy.abs();let d=if len>1.{(dx*(q[0][1]-q[1][1])-dy*(q[0][0]-q[1][0])).abs()+(dx*(q[0][1]-q[2][1])-dy*(q[0][0]-q[2][0])).abs()}else{len=1.;(q[0][0]-q[1][0]).abs()+(q[0][1]-q[1][1]).abs()+(q[0][0]-q[2][0]).abs()+(q[0][1]-q[2][1]).abs()};
 if d<threshold*len||level==0{if out.len()>=4096{return Err("Source dash flat segment admission exceeded".into());}out.push(q[3]);}else{let(a,b)=half(q);flatten(a,threshold,level-1,out)?;flatten(b,threshold,level-1,out)?;}Ok(())
}
pub(super)fn paths(path:&PathContour,width:f64,density:f64)->Result<Vec<PathContour>,String>{
 if !width.is_finite()||width<=0.||!density.is_finite()||density<=0.{return Err("Invalid source curved dash admission".into());}
 let mut points=vec![path.start];let mut previous=path.start;for s in &path.segments{match s{Segment::Line(to)=>points.push(*to),Segment::Cubic{control1,control2,to}=>flatten([previous,*control1,*control2,*to],0.5/density,9,&mut points)?,}previous=match s{Segment::Line(to)|Segment::Cubic{to,..}=>*to};}
 let pattern=[4.*width,2.*width];let(mut index,mut pos,mut offset,mut begin)=(0usize,0.,0.,0.);let mut moveto=points[0];let mut pieces=Vec::<Vec<Point>>::new();let mut work=0;
 for pair in points.windows(2){let(a,b)=(pair[0],pair[1]);let len=(b[0]-a[0]).hypot(b[1]-a[1]);let stop=begin+len;let mut done=pos>=stop;while !done{work+=1;if work>100_000{return Err("Source curved dash traversal admission exceeded".into());}let carry=offset>0.;let on=index==0;let dpos=(pos+pattern[index])-(offset+begin);if !dpos.is_finite()||dpos<0.{return Err("Source curved dash progress invalid".into());}let p=if dpos>len{offset=pattern[index]-(dpos-len);pos=stop;done=true;b}else{let p=mix(a,b,dpos/len);pos=dpos+begin;done=pos>=stop;index=(index+1)%2;offset=0.;p};if on{if !carry||pieces.is_empty(){pieces.push(vec![moveto]);}if pieces.last().unwrap().last()!=Some(&p){pieces.last_mut().unwrap().push(p);}}moveto=p;}begin=stop;}
 pieces.into_iter().filter(|p|p.len()>1).map(|p|stroke(&p,width/2.)).collect()
}
