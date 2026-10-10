//! Source comparison preparation. GPU depth resolution remains separately admitted.
//! The source compares a Qt float32 forward-distance against a Python float64
//! threshold. Rounding that threshold to nearest float32 changes boundary results.
pub(crate) fn source_threshold(along_point: f32) -> Result<f32, &'static str> {
    if !along_point.is_finite() { return Err("Nonfinite annotation forward distance"); }
    let point = along_point as f64;
    let threshold = point - 0.02_f64.max(0.003 * point.abs());
    Ok(ceil_f32(threshold))
}
fn ceil_f32(value: f64) -> f32 {
    let nearest = value as f32;
    if (nearest as f64) < value { nearest.next_up() } else { nearest }
}
/// CPU-prepared source query. Coordinates use the captured GL bottom-origin grid;
/// the GPU sampler must translate rows if its retained depth attachment is top-origin.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct Query {
    pub pixel: [i32; 2],
    pub normalized: [f32; 2],
    pub threshold: f32,
    pub active: u32,
    pub _padding: [u32; 2],
}
pub(crate) fn prepare_query(mvp: &[f32;16], world: [f32;3], eye: [f32;3],
    forward: [f32;3], extent: [u32;2]) -> Result<Query, &'static str> {
    if extent.contains(&0) || extent.iter().any(|&v|v>i32::MAX as u32) {
        return Err("Invalid annotation depth extent");
    }
    if !mvp.iter().chain(world.iter()).chain(eye.iter()).chain(forward.iter()).all(|v|v.is_finite()) {
        return Err("Nonfinite annotation query");
    }
    let mut result=Query::zeroed();
    let c=map(mvp,[world[0],world[1],world[2],1.]);
    if !c.iter().all(|v|v.is_finite()) {return Err("Annotation projection overflow");}
    if (c[3] as f64) <= 1e-9 {return Ok(result);}
    // Python promotes the Qt float components to double for division/pixel math.
    let nx=c[0] as f64/c[3] as f64;
    let ny=c[1] as f64/c[3] as f64;
    let px=(nx+1.)*0.5*extent[0] as f64;
    let py=(ny+1.)*0.5*extent[1] as f64;
    let ix=px.trunc();let iy=py.trunc();
    if ix<0. || iy<0. || ix>=extent[0] as f64 || iy>=extent[1] as f64 {return Ok(result);}
    let delta=[world[0]-eye[0],world[1]-eye[1],world[2]-eye[2]];
    let along=delta[0]*forward[0]+delta[1]*forward[1]+delta[2]*forward[2];
    result.pixel=[ix as i32,iy as i32];
    // QMatrix4x4::map receives QVector4D, which narrows normalized values.
    result.normalized=[nx as f32,ny as f32];
    result.threshold=source_threshold(along)?;
    result.active=1;
    Ok(result)
}
pub(crate) fn map(m: &[f32;16], v:[f32;4])->[f32;4] {
    std::array::from_fn(|r|m[r]*v[0]+m[4+r]*v[1]+m[8+r]*v[2]+m[12+r]*v[3])
}
/// Diagnostic-only CPU reconstruction for comparison with frozen Qt operation
/// cases. Production visibility reads depth on GPU; this never reads a frame.
fn sampled_occluded(query:Query, depth:f32, inverse:&[f32;16], eye:[f32;3], forward:[f32;3])->bool {
    if query.active==0 || depth>=1. {return false;}
    let q=map(inverse,[query.normalized[0],query.normalized[1],(depth as f64*2.-1.) as f32,1.]);
    if (q[3] as f64).abs()<1e-12{return false;}
    let surface=std::array::from_fn::<_,3,_>(|i|(q[i] as f64/q[3] as f64) as f32-eye[i]);
    let along=surface[0]*forward[0]+surface[1]*forward[1]+surface[2]*forward[2];
    along<query.threshold
}
use bytemuck::Zeroable;
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gpu_float_comparison_preserves_source_double_boundary() {
        for point in [-1e30_f32,-1000.,-1.,-0.021,0.,0.019,0.021,1.,100.,1000.,1e30] {
            let source = point as f64 - 0.02_f64.max(0.003 * (point as f64).abs());
            let boundary = source_threshold(point).unwrap();
            for surface in [boundary.next_down(),boundary,boundary.next_up(),point] {
                assert_eq!(surface < boundary, (surface as f64) < source,
                    "point={point:?}, surface={surface:?}, source={source:?}");
            }
        }
        assert!(source_threshold(f32::NAN).is_err());
        assert!(source_threshold(f32::INFINITY).is_err());
    }
    #[test]
    fn nearest_rounding_would_hide_a_source_visible_boundary() {
        let threshold = 1.0_f64 + f32::EPSILON as f64 * 0.25;
        let nearest = threshold as f32;
        assert!((nearest as f64) < threshold);
        assert!(!(nearest < nearest));
        assert!(nearest < ceil_f32(threshold));
    }
    #[test]
    fn inverse_w_shader_literal_is_the_source_double_ceiling() {
        let nearest = 1e-12_f64 as f32;
        let boundary = ceil_f32(1e-12_f64);
        assert_eq!(nearest.to_bits(), 0x2b8cbccc);
        assert_eq!(boundary.to_bits(), 0x2b8cbccd);
        for magnitude in [nearest.next_down(), nearest, boundary, boundary.next_up()] {
            for w in [magnitude, -magnitude] {
                assert_eq!(w.abs() < boundary, (w as f64).abs() < 1e-12_f64);
            }
        }
        for shader in [
            include_str!("shaders/annotation_visibility.comp"),
            include_str!("shaders/annotation_visibility_single.comp"),
        ] {
            assert!(shader.contains("uintBitsToFloat(0x2b8cbccdu)"));
        }
    }
    #[test]
    fn source_negative_fraction_is_admitted_but_whole_negative_pixel_is_not() {
        let m=glam::Mat4::IDENTITY.to_cols_array();
        let a=prepare_query(&m,[-1.1,0.,0.2],[0.;3],[0.,0.,1.],[5,5]).unwrap();
        assert_eq!(a.active,1);assert_eq!(a.pixel,[0,2]);
        let b=prepare_query(&m,[-1.5,0.,0.2],[0.;3],[0.,0.,1.],[5,5]).unwrap();
        assert_eq!(b.active,0);
        assert!(prepare_query(&m,[f32::NAN,0.,0.],[0.;3],[0.,0.,1.],[5,5]).is_err());
    }

    #[test]
    fn retained_actual_qt_depth_cases_match_prepared_queries_and_reconstruction() {
        let oracle:serde_json::Value=serde_json::from_str(include_str!("../../../../docs/audits/native-annotation-source/depth-original.json")).unwrap();
        let array=|v:&serde_json::Value|v.as_array().unwrap().iter().map(|x|x.as_f64().unwrap() as f32).collect::<Vec<_>>();
        let rows=oracle["cases"].as_array().unwrap();assert_eq!(rows.len(),18);
        for row in rows {
            if !row["fallback"].is_null(){assert_eq!(row["fallback_calls"].as_array().unwrap().len(),1);continue;}
            let m: [f32;16]=array(&row["mvp_column_major"]).try_into().unwrap();
            let inverse:[f32;16]=array(&row["inverse_mvp_column_major"]).try_into().unwrap();
            let world:[f32;3]=array(&row["world"]).try_into().unwrap();
            let eye:[f32;3]=array(&row["eye"]).try_into().unwrap();
            let forward:[f32;3]=array(&row["forward"]).try_into().unwrap();
            let query=prepare_query(&m,world,eye,forward,[5,5]).unwrap();
            let mut depth=0_f32;
            if query.active!=0 {
                for y in (query.pixel[1]-1).max(0)..=(query.pixel[1]+1).min(4) {
                    for x in (query.pixel[0]-1).max(0)..=(query.pixel[0]+1).min(4) {
                        depth=depth.max(row["depth"][y as usize][x as usize].as_f64().unwrap() as f32);
                    }
                }
            }
            assert_eq!(sampled_occluded(query,depth,&inverse,eye,forward),row["occluded"].as_bool().unwrap(),"{}",row["name"]);
        }
    }

}
