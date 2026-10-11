//! Cold, literal closed-iso diagnostic admission. No geometry is supplied to rendering.
use super::*;
use glam::{Mat4,Vec3,Vec4};
fn read<T:Copy>(buffer:&Buffer,index:usize)->Result<T,String>{
    if buffer.raw.storageMode()!=MTLStorageMode::Shared{return Err("Positive depth probe requires immutable Shared upload records".into());}
    let size=std::mem::size_of::<T>();let offset=index.checked_mul(size).filter(|o|o.checked_add(size).is_some_and(|end|end<=buffer.length())).ok_or("Positive depth buffer record outside admission")?;
    Ok(unsafe{buffer.raw.contents().as_ptr().cast::<u8>().add(offset).cast::<T>().read_unaligned()})
}
fn point(camera:Mat4,p:Vec3,position_error:f64)->Result<f32,String>{
    if !p.is_finite(){return Err("Positive depth submitted position is nonfinite".into());}
    let q=camera*p.extend(1.);
    let row=[camera.x_axis.z,camera.y_axis.z,camera.z_axis.z,camera.w_axis.z];
    let v=[p.x,p.y,p.z,1.];
    let terms=std::array::from_fn::<_,4,_>(|i|row[i]as f64*v[i]as f64);
    let ideal=terms.iter().sum::<f64>();
    // Conservative exclusion margin, not a rendered depth correction: permits
    // alternative FMA/association and bounded CPU/GPU contour arithmetic.
    let error=32.*f32::EPSILON as f64*terms.iter().map(|v|v.abs()).sum::<f64>()
        +position_error*row[..3].iter().map(|v|v.abs()as f64).sum::<f64>();
    if !q.is_finite() || q.w!=1. || q.z<0. || ideal-error<0.{return Err(format!("Positive depth clip domain refusal: clip={q:?}, lower_bound={}",ideal-error));}
    Ok(q.z)
}
pub(super) fn validate(frame:&Prepared)->Result<serde_json::Value,String>{
    if frame.overlay.is_some() || frame.background.is_some() || frame.mesh.is_some()
       || frame.lines.iter().flatten().any(|l|l.count!=0) || frame.profiles.as_ref().is_some_and(|p|p.count!=0)
       || frame.ghosts.as_ref().is_some_and(|g|g.bytes!=0) || frame.billboards.as_ref().is_some_and(|b|b.bytes!=0){return Err("Positive depth probe excludes extra submitted streams".into());}
    let placements=frame.placements.as_ref().ok_or("Positive depth probe requires placements")?;
    if placements.meshes.len()!=1 || placements.packet.bindings.len()!=1 || frame.target.poses.len()!=1{return Err("Positive depth probe requires literal single identity placement".into());}
    let models=frame.target.instance_matrices.as_ref().ok_or("Positive depth models absent")?;
    let model:[[f32;4];4]=read(models,0)?;
    if model!=Mat4::IDENTITY.to_cols_array_2d(){return Err("Positive depth probe model is not literal identity".into());}
    let camera=Mat4::from_cols_array_2d(&frame.mvp);
    if [camera.x_axis.w,camera.y_axis.w,camera.z_axis.w,camera.w_axis.w]!=[0.,0.,0.,1.]{return Err("Positive depth probe camera is not affine w1".into());}
    if frame.section.plane_for_probe()!=[0.,0.,-1.,1.5] || !frame.section.enabled(){return Err("Positive depth probe requires literal section plane".into());}
    let mesh=placements.meshes[0].as_ref().ok_or("Positive depth mesh absent")?;
    if mesh.count!=60 || mesh.materials.as_ref().is_some_and(|m|!m.runs.is_empty()||!m.textures.is_empty()||m.colored_vertices.is_some()){return Err("Positive depth probe requires literal opaque60-index mesh".into());}
    let vertices=mesh.vertices.as_ref().ok_or("Positive depth vertices absent")?;
    let indices=mesh.indices.as_ref().ok_or("Positive depth indices absent")?;
    let vertex_size=std::mem::size_of::<ingetrazo_render::buffer::Vertex>();
    if vertices.length()%vertex_size!=0 || vertices.length()/vertex_size>4096{return Err("Positive depth vertex receipt exceeds bounded domain".into());}
    let count=vertices.length()/vertex_size;let mut min_z=f32::INFINITY;let mut original_vertices=0;
    for i in 0..count{let v:ingetrazo_render::buffer::Vertex=read(vertices,i)?;let p=Vec3::from_array(v.position);min_z=min_z.min(point(camera,p,0.)?);original_vertices+=1;}
    let fill=frame.fill.as_ref().ok_or("Positive depth cap absent")?;
    if fill.vertices.length()!=6*vertex_size{return Err("Positive depth cap stream length differs".into());}
    for i in 0..6{let v:ingetrazo_render::buffer::Vertex=read(&fill.vertices,i)?;min_z=min_z.min(point(camera,Vec3::from_array(v.position),0.)?);}
    if frame.contour.is_none(){return Err("Positive depth contour stream absent".into());}
    let plane=-Vec4::from_array(frame.section.plane_for_probe());
    let eye=Vec3::from_array(frame.contour_eye_width[..3].try_into().unwrap());let scale=frame.contour_eye_width[3];
    if !eye.is_finite()||!scale.is_finite()||scale<0.{return Err("Positive depth contour parameters invalid".into());}
    let mut active=0;let mut fallback=0;
    for triangle in 0..mesh.count/3{
        let mut p=[Vec3::ZERO;3];let mut d=[0.;3];
        for i in 0..3{let index:u32=read(indices,triangle*3+i)?;let v:ingetrazo_render::buffer::Vertex=read(vertices,index as usize)?;p[i]=Vec3::from_array(v.position);d[i]=p[i].extend(1.).dot(plane);}
        let mut hits=[Vec3::ZERO;3];let mut n=0;
        for i in 0..3{let k=(i+1)%3;if d[i]*d[k]<0.{hits[n]=p[i]+(p[k]-p[i])*(d[i]/(d[i]-d[k]));n+=1;}}
        let tangent=hits[1]-hits[0];let length=tangent.length();
        if n!=2 || length<=1e-9{fallback+=1;continue;}
        let normal=plane.truncate();let side=normal.cross(tangent/length);
        let width=(((hits[0]+hits[1])*0.5-eye).length()*scale).max(1.25e-6);let lift=normal*(0.6*width);
        let quad=[hits[0]-side*width+lift,hits[1]-side*width+lift,hits[1]+side*width+lift,hits[0]+side*width+lift];
        let magnitude=p.iter().map(|v|v.abs().max_element()as f64).fold(eye.abs().max_element()as f64,f64::max).max(width as f64).max(1.);
        let position_error=128.*f32::EPSILON as f64*magnitude;
        for q in quad{min_z=min_z.min(point(camera,q,position_error)?);}active+=1;
    }
    Ok(serde_json::json!({"enabled":true,"all_original_and_cap_and_active_contour_clip_z_nonnegative":true,"all_w_exactly1":true,"x_y_w_unchanged":true,"min_unremapped_clip_z":min_z,"original_vertices":original_vertices,"cap_vertices":6,"active_contour_vertices":active*6,"clipped_contour_fallback_vertices":fallback*6,"fallback_clip_position":[2,2,2,1],"viewport_depth_range":[0.5,1.0],"guard_scope":"Literal immutable Shared streams and conservative active contour quad exclusion margin; no CPU geometry rendered"}))
}
