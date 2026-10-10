//! Source walkthrough movement using the existing orbit camera and visible-face ray queries.
use crate::camera::OrbitCamera;
use glam::DVec3;

#[derive(Default, Clone, Copy, Debug)]
pub struct WalkModifiers { pub run: bool, pub shift: bool, pub through_walls: bool }
pub const DEFAULT_EYE_HEIGHT: f64 = 1.68;
pub const CLEARANCE: f64 = 0.35;
pub const STEP_UP: f64 = 0.5;
pub const KNEE: f64 = 0.55;

fn heading(camera: &OrbitCamera) -> DVec3 {
    let d = camera.forward();
    let horizontal = DVec3::new(d.x,d.y,0.);
    if horizontal.length() > 1e-6 { horizontal.normalize() } else { DVec3::Y }
}
fn gain(px: f64) -> f64 { if px.abs() < 4. { 0. } else { (px/150.).clamp(-1.,1.) } }
fn blocked(ray: &mut impl FnMut(DVec3,DVec3)->Option<f64>, eye:DVec3, delta:DVec3,height:f64)->bool {
    let length=delta.length();
    if length < 1e-12 { return false; }
    let direction=delta/length;
    [eye,eye-DVec3::Z*(height-KNEE).max(0.)].into_iter().any(|origin|
        ray(origin,direction).is_some_and(|t|t < length+CLEARANCE))
}
/// Deterministic source Walk step. The GUI supplies its tick duration and current modifiers.
/// Ray queries must return distance to the nearest visible face, using the shared pick index.
pub fn step(camera:&mut OrbitCamera,dx:f64,dy:f64,dt:f64,height:f64,modifiers:WalkModifiers,
    mut ray:impl FnMut(DVec3,DVec3)->Option<f64>)->bool {
    if ![dx,dy,dt,height].into_iter().all(f64::is_finite) || dt<0. || height<=0. { return false; }
    let before=camera.clone();
    let speed=1.5*if modifiers.run {3.} else {1.};
    let eye=camera.eye();
    let mut forward=heading(camera);
    let right=forward.cross(DVec3::Z).normalize();
    if modifiers.shift {
        let delta=right*(gain(dx)*speed*dt)+DVec3::Z*(gain(dy)*speed*dt);
        if delta.length_squared()<1e-18 { return false; }
        if !modifiers.through_walls && blocked(&mut ray,eye,delta,height) { return false; }
        return camera.move_eye(delta);
    }
    let turn=gain(dx)*60.*dt;
    if turn.abs()>1e-9 { camera.turn(turn,0.); forward=heading(camera); }
    let advance=gain(dy)*speed*dt;
    if advance.abs()<1e-12 { return *camera!=before; }
    let delta=forward*advance;
    if !modifiers.through_walls && blocked(&mut ray,eye,delta,height) { return *camera!=before; }
    if !camera.move_eye(delta) { *camera=before; return false; }
    if !modifiers.through_walls {
        let eye=camera.eye();
        let z_from=eye.z-height+STEP_UP;
        let floor=ray(DVec3::new(eye.x,eye.y,z_from),-DVec3::Z)
            .map(|distance|z_from-distance).unwrap_or(0_f64.min(eye.z-height));
        let target_z=floor+height;
        if (target_z-eye.z).abs()>1e-6 && !camera.move_eye(DVec3::Z*(target_z-eye.z)) {
            *camera=before;return false;
        }
    }
    *camera!=before
}

/// Source FirstPersonTool.step; simultaneous axes share one normalized movement speed.
/// Running uses Shift in the GUI; Ctrl shortcuts remain owned by the application.
pub fn first_person_step(camera:&mut OrbitCamera,axes:[i8;3],dt:f64,height:f64,run:bool,through_walls:bool,
 mut ray:impl FnMut(DVec3,DVec3)->Option<f64>)->bool {
 if !dt.is_finite()||dt<0.||!height.is_finite()||height<=0. {return false;}
 if axes==[0,0,0]{return false;}
 let before=camera.clone();let forward=heading(camera);let right=forward.cross(DVec3::Z).normalize();
 let direction=forward*axes[0]as f64+right*axes[1]as f64+DVec3::Z*axes[2]as f64;
 let delta=direction.normalize_or_zero()*(1.5*if run{3.}else{1.}*dt);
 if !through_walls&&blocked(&mut ray,camera.eye(),delta,height){return false;}
 if !camera.move_eye(delta){return false;}
 if !through_walls&&axes[2]==0 {
  let eye=camera.eye();let z_from=eye.z-height+STEP_UP;
  let floor=ray(DVec3::new(eye.x,eye.y,z_from),-DVec3::Z).map(|distance|z_from-distance).unwrap_or(0_f64.min(eye.z-height));
  let target_z=floor+height;
  if (target_z-eye.z).abs()>1e-6&&!camera.move_eye(DVec3::Z*(target_z-eye.z)){*camera=before;return false;}
 }
 *camera!=before
}

#[cfg(test)]
mod tests {
    use super::*;
    fn camera()->OrbitCamera { let mut c=OrbitCamera::new();c.look_from(DVec3::Z*DEFAULT_EYE_HEIGHT,DVec3::Y);c }
    #[test] fn wall_clearance_stops_forward_motion_but_alt_passes() {
        let mut c=camera();let before=c.clone();
        let wall=|eye:DVec3,d:DVec3| if d.y>0. { Some((0.4-eye.y)/d.y) } else {None};
        assert!(!step(&mut c,0.,150.,0.1,DEFAULT_EYE_HEIGHT,WalkModifiers::default(),wall));assert_eq!(c,before);
        assert!(step(&mut c,0.,150.,0.1,DEFAULT_EYE_HEIGHT,WalkModifiers{through_walls:true,..Default::default()},wall));
        assert!((c.eye().y-0.15).abs()<1e-12);
    }
    #[test] fn shift_vertical_motion_does_not_follow_floor() {
        let mut c=camera();step(&mut c,0.,150.,0.1,DEFAULT_EYE_HEIGHT,WalkModifiers{shift:true,..Default::default()},|_,_|None);
        assert!((c.eye().z-(DEFAULT_EYE_HEIGHT+0.15)).abs()<1e-12);
    }
    #[test] fn refused_step_preserves_camera() {
        let mut c=camera();let before=c.clone();assert!(!step(&mut c,f64::NAN,0.,0.1,DEFAULT_EYE_HEIGHT,Default::default(),|_,_|None));assert_eq!(c,before);
    }
}
