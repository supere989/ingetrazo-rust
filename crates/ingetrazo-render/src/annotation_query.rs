//! Source comparison preparation. GPU depth resolution remains separately admitted.
//! The source compares a Qt float32 forward-distance against a Python float64
//! threshold. Rounding that threshold to nearest float32 changes boundary results.
pub fn source_threshold(along_point: f32) -> Result<f32, &'static str> {
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
pub struct Query {
    pub pixel: [i32; 2],
    pub normalized: [f32; 2],
    pub threshold: f32,
    pub active: u32,
    pub _padding: [u32; 2],
}
pub fn prepare_query(mvp: &[f32;16], world: [f32;3], eye: [f32;3],
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
pub fn map(m: &[f32;16], v:[f32;4])->[f32;4] {
    std::array::from_fn(|r|m[r]*v[0]+m[4+r]*v[1]+m[8+r]*v[2]+m[12+r]*v[3])
}

use bytemuck::Zeroable;
