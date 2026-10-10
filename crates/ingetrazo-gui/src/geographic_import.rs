use crate::IngeTrazoApp;
use ingetrazo_core::{camera::StandardView,georef::{SceneDatum,GeoPath,TileCapture}};
use glam::DVec3;
impl IngeTrazoApp{
 pub fn import_geographic_paths(&mut self,path:&std::path::Path)->Result<usize,String>{
  let features=ingetrazo_formats::geoimport::load_features(path)?;
  if features.is_empty(){self.status_message="No lines or polygons found in the file.".into();return Ok(0)}
  let datum=match &self.scene.geo_datum{Some(d)=>d.clone(),None=>{
   let count=features.iter().map(|f|f.points.len()).sum::<usize>();
   fn source_sum(values:impl Iterator<Item=f64>)->f64{
    let mut hi:f64=0.;let mut lo=0.;
    for x in values{let total=hi+x;lo+=if hi.abs()>=x.abs(){(hi-total)+x}else{(x-total)+hi};hi=total;}
    hi+lo
   }
   let lat=source_sum(features.iter().flat_map(|f|f.points.iter().map(|p|p[0])));
   let lon=source_sum(features.iter().flat_map(|f|f.points.iter().map(|p|p[1])));
   SceneDatum::new(lat/count as f64,lon/count as f64,0.,0.)
  }};
  let mut imported=Vec::with_capacity(features.len());
  for f in features{
   let points:Vec<_>=f.points.iter().map(|p|datum.geodetic_to_local(p[0],p[1],None).as_vec3().as_dvec3()).collect();
   if points.iter().any(|p|!p.is_finite()){return Err("Imported geographic coordinates are not finite".into())}
   imported.push(GeoPath::new(points,f.closed,f.name));
  }
  let mut camera=self.camera.clone();camera.set_standard_view(StandardView::Top);
  let mut min=DVec3::splat(f64::INFINITY);let mut max=DVec3::splat(f64::NEG_INFINITY);
  for p in self.scene.geo_paths.iter().chain(&imported).flat_map(|p|&p.points){min=min.min(*p);max=max.max(*p)}
  min.z=0.;max.z=0.;
  camera.target=((min+max)*0.5).as_vec3().as_dvec3();
  // Source QVector3D::length uses the scaled three-argument float hypot.
  // Keep its f32 operation order; glam's unscaled dot/sqrt rounds differently.
  let span=(max-min).as_vec3().abs();let scale=span.max_element();
  let length=if scale==0. {0.}else{let q=span/scale;scale*((q.x*q.x+q.y*q.y)+q.z*q.z).sqrt()};
  let diagonal=length.max(0.02) as f64;
  camera.distance=(diagonal*1.3/(2.*(camera.fov_deg.to_radians()*0.5).tan())).max(0.02);
  // Source datum initialization and viewport framing are outside the path command.
  // Validate first, then retain both in the command's before-state.
  if !(1..=21).contains(&self.base_map_requested_zoom){return Err("Base-map zoom outside supported UI domain".into())}
  let capture=TileCapture::import_reference(self.scene.tile_layer.as_ref(),&datum,min,max,self.base_map_requested_zoom,self.base_map_requested_visible);
  self.scene.tile_layer=Some(capture);self.scene.geo_datum=Some(datum);self.camera=camera.clone();self.scene.camera=camera;
  self.save_undo_state();
  let count=imported.len();self.scene.geo_paths.extend(imported);self.scene.bump_version();
  self.status_message=format!("Imported {count} feature(s) from {}",path.file_name().unwrap_or_default().to_string_lossy());
  Ok(count)
 }
}
