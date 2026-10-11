//! Qt 6.12 QTriangulatingStroker float-coordinate primitive construction.
//! No coverage rasterization: these immutable vertices are intended for the
//! existing single-sample annotation attachment, with opaque layer admission.

/// Separate source casts/additions matter: Qt stores each coordinate and the
/// normal in float before constructing its square-cap triangle strip.
/// Keep both original endpoint pairs: collapsing the eight source vertices
/// into four outer corners changes float edge segments at narrow strokes.
pub(crate) fn square_cap_line(a: [f64; 2], b: [f64; 2], width: f64) -> Result<Option<[[f32; 2]; 8]>, String> {
    if !a.into_iter().chain(b).chain([width]).all(f64::is_finite) || width < 0. {
        return Err("Invalid widget line input".into());
    }
    let a = a.map(|v| v as f32);
    let b = b.map(|v| v as f32);
    let width = if width == 0. { 1. } else { width as f32 };
    if !a.into_iter().chain(b).chain([width]).all(f32::is_finite) {
        return Err("Widget line float conversion overflow".into());
    }
    // Qt removes consecutive points equal after conversion to float.
    if a == b { return Ok(None); }
    let dx = b[0] - a[0];
    let dy = b[1] - a[1];
    let half = width / 2.;
    let scale = half / dx.hypot(dy);
    let nx = -dy * scale;
    let ny = dx * scale;
    let start = [a[0] - ny, a[1] + nx];
    let end = [b[0] + ny, b[1] - nx];
    let vertices = [
        [start[0] + nx, start[1] + ny],
        [start[0] - nx, start[1] - ny],
        [a[0] + nx, a[1] + ny],
        [a[0] - nx, a[1] - ny],
        [b[0] + nx, b[1] + ny],
        [b[0] - nx, b[1] - ny],
        [end[0] + nx, end[1] + ny],
        [end[0] - nx, end[1] - ny],
    ];
    if !vertices.iter().flatten().all(|v| v.is_finite()) {
        return Err("Widget line float arithmetic overflow".into());
    }
    Ok(Some(vertices))
}

/// Qt's OpenGL ellipse fill samples the four original qreal cubics, not the
/// raster paint engine's adaptive flattening. Bound every allocation before
/// sampling; the fixed source cap is 64 points per cubic (257 fan vertices).
pub(crate) fn ellipse_fill(center:[f64;2],radius:[f64;2],density:f64)->Result<Vec<[f32;2]>,String>{
 if !center.into_iter().chain(radius).chain([density]).all(f64::is_finite)||radius.iter().any(|&r|r<0.)||density<=0.||density>40.{return Err("Widget ellipse source input invalid".into());}
 if radius==[0.;2]{return Ok(Vec::new());}
 let control=ellipse_controls(center,radius)?;
 let mut result=Vec::new();result.try_reserve_exact(257).map_err(|_|"Widget ellipse fan allocation failed")?;result.push(control[0].map(|v|v as f32));
 for quarter in 0..4{
  let p=&control[quarter*3..quarter*3+4];
  let span=(0..2).map(|axis|p.iter().map(|p|p[axis]).fold(f64::NEG_INFINITY,f64::max)-p.iter().map(|p|p[axis]).fold(f64::INFINITY,f64::min)).fold(0f64,f64::max);
  // addPath takes inverseScale as GLfloat; qMin<float> narrows only
  // after the qreal bounds multiplication and float denominator arithmetic.
  let inverse=(1./density)as f32;let n=((span*f64::from(3.14f32)/f64::from(inverse*6.))as f32).min(64.).max(3.)as usize;
  let step=1./(n-1)as f64;
  for i in 0..n{
   let t=i as f64*step;let u=1.-t;
   let point=std::array::from_fn(|axis|{
    let a=p[0][axis]*u+p[1][axis]*t;let b=p[1][axis]*u+p[2][axis]*t;let c=p[2][axis]*u+p[3][axis]*t;
    let a=a*u+b*t;let b=b*u+c*t;(a*u+b*t)as f32
   });result.push(point);
  }
 }
 if result.last()!=result.first(){result.push(result[0]);}
 // Last close can only add one, and the t=1 endpoint equals the start.
 if result.len()>257{return Err("Widget ellipse fan exceeds source segment cap".into());}Ok(result)
}

fn ellipse_controls(center:[f64;2],radius:[f64;2])->Result<[[f64;2];13],String>{
 let x=center[0]-radius[0];let y=center[1]-radius[1];let w=radius[0]*2.;let h=radius[1]*2.;let rx=w/2.;let ry=h/2.;let k=0.5522847498;let kx=rx*k;let ky=ry*k;
 let control=[[x+w,y+ry],[x+w,y+ry+ky],[x+rx+kx,y+h],[x+rx,y+h],
  [x+rx-kx,y+h],[x,y+ry+ky],[x,y+ry],[x,y+ry-ky],[x+rx-kx,y],
  [x+rx,y],[x+rx+kx,y],[x+w,y+ry-ky],[x+w,y+ry]];
 if control.iter().flatten().any(|v|!v.is_finite()||v.abs()>1_000_000.){return Err("Widget ellipse control bounds exceed arithmetic domain".into());}
 Ok(control)
}
/// Bevel-joined closed cubic stroker, with float normals and the distinct
/// source stroke segmentation rule. Closed curves use flat caps.
pub(crate) fn ellipse_stroke(center:[f64;2],radius:[f64;2],width:f64,density:f64)->Result<Vec<[f32;2]>,String>{
 if !center.into_iter().chain(radius).chain([width,density]).all(f64::is_finite)||radius.iter().any(|&r|r<0.)||width<0.||density<=0.||density>40.{return Err("Widget ellipse stroke source input invalid".into());}
 if radius==[0.;2]{return Ok(Vec::new());}
 let control=ellipse_controls(center,radius)?;let inverse=1./density;let real_width=if width==0.{1.}else{width as f32};
 let half=if width==0.{(f64::from(real_width/2.)*inverse)as f32}else{real_width/2.};
 if !half.is_finite(){return Err("Widget ellipse stroke width exceeds float domain".into());}
 let flatness=std::f64::consts::PI/8.;
 let (add,mul)=if real_width<2.5&&(width==0.||inverse==1.){(0.5,(flatness/inverse)as f32)}else if width==0.{(real_width/2.,flatness as f32)}else{(half,(flatness/inverse)as f32)};
 let normal=|a:[f32;2],b:[f32;2]|{let dx=b[0]-a[0];let dy=b[1]-a[1];let scale=half/dx.hypot(dy);[-dy*scale,dx*scale]};
 let mut vertices=Vec::new();vertices.try_reserve_exact(515).map_err(|_|"Widget ellipse stroke allocation failed")?;
 let emit=|vertices:&mut Vec<[f32;2]>,p:[f32;2],n:[f32;2]|{vertices.push([p[0]+n[0],p[1]+n[1]]);vertices.push([p[0]-n[0],p[1]-n[1]]);};
 let mut current=control[0].map(|v|v as f32);emit(&mut vertices,current,normal(current,control[1].map(|v|v as f32)));
 for quarter in 0..4{
  let p=&control[quarter*3..quarter*3+4];
  if quarter!=0{emit(&mut vertices,current,normal(current,p[1].map(|v|v as f32)));}
  let span=(0..2).map(|axis|p.iter().map(|p|p[axis]).fold(f64::NEG_INFINITY,f64::max)-p.iter().map(|p|p[axis]).fold(f64::INFINITY,f64::min)).fold(0f64,f64::max)as f32;
  let n=((span+add)*mul).min(64.).max(4.)as usize;
  for i in 1..n{
   let t=i as f64/(n-1)as f64;let u=1.-t;
   let point=std::array::from_fn(|axis|{let a=p[0][axis]*u+p[1][axis]*t;let b=p[1][axis]*u+p[2][axis]*t;let c=p[2][axis]*u+p[3][axis]*t;let a=a*u+b*t;let b=b*u+c*t;(a*u+b*t)as f32});
   let v=normal(current,point);emit(&mut vertices,point,v);current=point;
  }
 }
 emit(&mut vertices,current,normal(current,control[1].map(|v|v as f32)));vertices.push(*vertices.last().unwrap());
 if vertices.len()>515||vertices.iter().flatten().any(|v|!v.is_finite()||v.abs()>1_000_000.){return Err("Widget ellipse stroke exceeds source allocation/arithmetic domain".into());}Ok(vertices)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_narrow_strip_matches_actual_qt_vbo() {
        let expected = [
            [20.02494239807129,20.602949142456055],
            [20.147050857543945,20.27494239807129],
            [20.188945770263672,20.664003372192383],
            [20.311054229736328,20.335996627807617],
            [180.68894958496094,80.41400146484375],
            [180.81105041503906,80.08599853515625],
            [180.8529510498047,80.47505950927734],
            [180.9750518798828,80.14705657958984],
        ];
        let actual = square_cap_line([20.25,20.5],[180.75,80.25],0.35).unwrap().unwrap();
        assert_eq!(actual.map(|p|p.map(f32::to_bits)), expected.map(|p:[f32;2]|p.map(f32::to_bits)));
    }
    #[test]
    fn square_cap_float_sequence_and_collapsed_source_points() {
        assert_eq!(square_cap_line([2., 3.], [8., 3.], 2.).unwrap().unwrap(),
            [[1., 4.], [1., 2.], [2., 4.], [2., 2.], [8., 4.], [8., 2.], [9., 4.], [9., 2.]]);
        assert_eq!(square_cap_line([2., 3.], [2., 3.], 2.).unwrap(), None);
        assert_eq!(square_cap_line([16_777_216., 3.], [16_777_217., 3.], 2.).unwrap(), None);
        assert!(square_cap_line([f64::MAX, 0.], [0., 0.], 1.).is_err());
        assert!(square_cap_line([-f32::MAX as f64, 0.], [f32::MAX as f64, 0.], 1.).is_err());
        assert!(square_cap_line([f64::NAN, 0.], [0., 0.], 1.).is_err());
    }
}
