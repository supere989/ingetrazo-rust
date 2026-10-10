//! Source maximal-hole policy with bounded native triangulation fallback.
//! Exact interior-triangle tie/rounding generality remains unproved.
use glam::{DVec3,Vec3};
#[derive(Default,Debug)]pub struct Metrics{pub pair_checks:usize,pub fallback_triangulations:usize}
fn div(v:Vec3,d:f32)->Vec3{Vec3::new(v.x/d,v.y/d,v.z/d)}
fn length(v:Vec3)->f32{let m=v.abs().max_element();if m==0.{0.}else{let n=div(v,m);m*(n.x*n.x+n.y*n.y+n.z*n.z).sqrt()}}
fn normalized(v:Vec3)->Vec3{let l=length(v);if (l-1.).abs()<1e-5{v}else if l.abs()<1e-5{Vec3::ZERO}else{div(v,l)}}
fn axes(normal:Vec3)->(Vec3,Vec3){let n=normalized(normal);let mut u=Vec3::X-n*n.x;if length(u)<0.1{u=Vec3::Y-n*n.y;}let u=normalized(u);(u,normalized(n.cross(u)))}
fn projected(p:Vec3,origin:Vec3,u:Vec3,v:Vec3)->(f64,f64){let r=p-origin;(r.dot(u)as f64,r.dot(v)as f64)}
fn contains(point:Vec3,outer:&[DVec3],normal:Vec3)->bool{if outer.len()<3{return false;}let(u,v)=axes(normal);let origin=outer[0].as_vec3();let(x,y)=projected(point,origin,u,v);let mut inside=false;let mut j=outer.len()-1;for i in 0..outer.len(){let(xi,yi)=projected(outer[i].as_vec3(),origin,u,v);let(xj,yj)=projected(outer[j].as_vec3(),origin,u,v);if ((yi>y)!=(yj>y))&&(x<(xj-xi)*(y-yi)/(yj-yi)+xi){inside=!inside;}j=i;}inside}
fn interior(inner:&[DVec3],normal:Vec3)->Option<Vec3>{let tris=ingetrazo_core::triangulate::triangulate(inner,&[],Some(normal.as_dvec3()));let mut best=None;let mut best_area=0f32;for(a,b,c)in tris{let(a,b,c)=(a.as_vec3(),b.as_vec3(),c.as_vec3());let cross=(b-a).cross(c-a);let area=cross.x*cross.x+cross.y*cross.y+cross.z*cross.z;if area>best_area{best_area=area;best=Some(div(a+b+c,3.));}}best}
pub fn maximal(holes:&[Vec<DVec3>])->Result<(Vec<usize>,Metrics),String>{let mut metrics=Metrics::default();let mut areas=Vec::new();areas.try_reserve_exact(holes.len()).map_err(|_|"Hole area preparation refused")?;let mut quantized=Vec::new();quantized.try_reserve_exact(holes.len()).map_err(|_|"Hole coordinate preparation refused")?;
 for ring in holes{let mut points=Vec::new();points.try_reserve_exact(ring.len()).map_err(|_|"Hole coordinate preparation refused")?;for p in ring{let p=p.as_vec3();if !p.is_finite(){return Err("Nonfinite hole coordinate".into());}points.push(p.as_dvec3());}let mut a=0.;for i in 0..points.len(){a+=points[i].x*points[(i+1)%points.len()].y;a-=points[(i+1)%points.len()].x*points[i].y;}areas.push(a.abs()/2.);quantized.push(points);}
 let mut order=Vec::new();order.try_reserve_exact(holes.len()).map_err(|_|"Hole area ordering refused")?;order.extend(0..holes.len());order.sort_unstable_by(|&a,&b|areas[a].total_cmp(&areas[b]).then(a.cmp(&b)));
 let mut keep=Vec::new();keep.try_reserve_exact(holes.len()).map_err(|_|"Hole filter result refused")?;
 for(i,h)in quantized.iter().enumerate(){if h.len()<3{return Err("Source maximal-hole normal requires three positions".into());}let n=(h[1].as_vec3()-h[0].as_vec3()).cross(h[2].as_vec3()-h[0].as_vec3());if (length(n)as f64)<1e-9{keep.push(i);continue;}let normal=normalized(n);let center=DVec3::new(h.iter().map(|p|p.x).sum::<f64>()/h.len()as f64,h.iter().map(|p|p.y).sum::<f64>()/h.len()as f64,h.iter().map(|p|p.z).sum::<f64>()/h.len()as f64).as_vec3();let mut cached=None;let mut nested=false;
  for &j in &order[order.partition_point(|&j|areas[j]<=areas[i]+1e-6)..]{let outer=&quantized[j];metrics.pair_checks+=1;if contains(center,outer,normal){nested=true;break;}let point=*cached.get_or_insert_with(||{metrics.fallback_triangulations+=1;interior(h,normal)});if point.is_some_and(|p|contains(p,outer,normal)){nested=true;break;}}
  if !nested{keep.push(i);}
 }Ok((keep,metrics))}

#[cfg(test)]mod tests{use super::*;
 #[test]fn equal_area_holes_skip_pair_search_and_fallbacks(){let square=vec![DVec3::ZERO,DVec3::X,DVec3::new(1.,1.,0.),DVec3::Y];let holes=vec![square;200];let(keep,m)=maximal(&holes).unwrap();assert_eq!(keep.len(),200);assert_eq!(m.pair_checks,0);assert_eq!(m.fallback_triangulations,0);}
 #[test]fn malformed_multi_hole_refuses_before_output(){assert!(maximal(&[vec![],vec![DVec3::ZERO,DVec3::X,DVec3::Y]]).unwrap_err().contains("requires three"));}
 #[test]fn disjoint_fallback_is_cached_per_inner(){let small=vec![DVec3::ZERO,DVec3::X,DVec3::new(1.,1.,0.),DVec3::Y];let medium=vec![DVec3::new(10.,0.,0.),DVec3::new(12.,0.,0.),DVec3::new(12.,2.,0.),DVec3::new(10.,2.,0.)];let large=vec![DVec3::new(20.,0.,0.),DVec3::new(23.,0.,0.),DVec3::new(23.,3.,0.),DVec3::new(20.,3.,0.)];let(keep,m)=maximal(&[small,medium,large]).unwrap();assert_eq!(keep,vec![0,1,2]);assert_eq!(m.pair_checks,3);assert_eq!(m.fallback_triangulations,2);}
}
