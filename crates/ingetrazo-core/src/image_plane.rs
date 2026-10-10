//! Display-only reference images; never topology or mesh export.
use glam::DVec3;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImagePlane {
    #[serde(skip, default = "uuid::Uuid::new_v4")]
    pub id: uuid::Uuid,
    pub path: String,
    pub origin: DVec3,
    pub u: DVec3,
    pub v: DVec3,
    pub aspect: f64,
    pub name: String,
    pub opacity: f64,
    pub layer: Option<String>,
    pub locked: bool,
    #[serde(default)]
    pub native_document: serde_json::Value,
    #[serde(default)]
    pub native_baseline: serde_json::Value,
}
impl PartialEq for ImagePlane {
    fn eq(&self, o: &Self) -> bool {
        self.path == o.path
            && self.origin == o.origin
            && self.u == o.u
            && self.v == o.v
            && self.aspect == o.aspect
            && self.name == o.name
            && self.opacity == o.opacity
            && self.layer == o.layer
            && self.locked == o.locked
            && self.native_document == o.native_document
            && self.native_baseline == o.native_baseline
    }
}
impl ImagePlane {
    pub fn new(path: impl Into<String>, origin: DVec3, u: DVec3, v: DVec3) -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            path: path.into(),
            origin:origin.as_vec3().as_dvec3(),
            u:u.as_vec3().as_dvec3(),
            v:v.as_vec3().as_dvec3(),
            aspect: 1.,
            name: String::new(),
            opacity: 1.,
            layer: Some("Images".into()),
            locked: false,
            native_document: serde_json::Value::Null,
            native_baseline: serde_json::Value::Null,
        }
    }
    pub fn corners(&self) -> [DVec3;4] {
        let (o,u,v)=(self.origin.as_vec3(),self.u.as_vec3(),self.v.as_vec3());
        [o.as_dvec3(),(o+u).as_dvec3(),(o+u+v).as_dvec3(),(o+v).as_dvec3()]
    }
    pub fn center(&self)->DVec3 {(self.origin.as_vec3()+(self.u.as_vec3()+self.v.as_vec3())*0.5).as_dvec3()}
    pub fn normal(&self)->DVec3 {
        let a=self.u.as_vec3();let b=self.v.as_vec3();let n=glam::Vec3::new(a.y*b.z-a.z*b.y,a.z*b.x-a.x*b.z,a.x*b.y-a.y*b.x);
        let len=n.length();if len>1e-12{if (len-1.).abs()<=1e-5{n.as_dvec3()}else if len.abs()<=1e-5{DVec3::ZERO}else{(n/len).as_dvec3()}}else{DVec3::Z}
    }
    /// Source rectangular projection intentionally uses separate dot fractions,
    /// including its non-orthogonal/degenerate frame behavior.
    pub fn ray_hit(&self,ray:&crate::math::Ray)->Option<f64>{
        fn dot(a:glam::Vec3,b:glam::Vec3)->f32{a.x*b.x+a.y*b.y+a.z*b.z}
        let n=self.normal().as_vec3();let (origin,direction)=(ray.origin.as_vec3(),ray.dir.as_vec3());
        let denom=dot(n,direction)as f64;if denom.abs()<1e-9{return None;}
        let t=dot(n,self.center().as_vec3()-origin)as f64/denom;if t<0.||!t.is_finite(){return None;}
        let d=origin+direction*(t as f32)-self.origin.as_vec3();let u=self.u.as_vec3();let v=self.v.as_vec3();let lu=dot(u,u)as f64;let lv=dot(v,v)as f64;
        let fu=if lu>1e-18{dot(d,u)as f64/lu}else{0.};let fv=if lv>1e-18{dot(d,v)as f64/lv}else{0.};
        (-1e-9..=1.+1e-9).contains(&fu).then_some(()).filter(|_|(-1e-9..=1.+1e-9).contains(&fv)).map(|_|t)
    }
    pub fn to_json(&self) -> serde_json::Value {
        let mut x = serde_json::json!({"texture":{"path":self.path},"origin":self.origin,"u":self.u,"v":self.v,"aspect":self.aspect});
        if !self.name.is_empty() {
            x["name"] = serde_json::json!(self.name);
        }
        if self.opacity != 1. {
            x["opacity"] = serde_json::json!(self.opacity);
        }
        match self.layer.as_deref() {
            None => x["layer"] = serde_json::Value::Null,
            Some("Images" | "") => {}
            Some(name) => x["layer"] = serde_json::json!(name),
        }
        if self.locked {
            x["locked"] = serde_json::json!(true);
        }
        x
    }
    pub fn from_json(raw: &serde_json::Value) -> Result<Self, String> {
        fn vector(raw: &serde_json::Value, k: &str) -> Result<DVec3, String> {
            let a = raw[k]
                .as_array()
                .ok_or_else(|| format!("Image plane {k} must be a vector"))?;
            if a.len() != 3 {
                return Err(format!("Image plane {k} must have three coordinates"));
            }
            let mut v = [0.; 3];
            for (i, x) in a.iter().enumerate() {
                v[i] = x
                    .as_f64()
                    .filter(|v| v.is_finite())
                    .ok_or_else(|| format!("Image plane {k} is not finite"))?
                    as f32 as f64;
            }
            Ok(DVec3::from_array(v))
        }
        let path = raw
            .get("texture")
            .filter(|v| v.is_object())
            .map(|v| v.get("embed").or_else(|| v.get("path")))
            .unwrap_or_else(|| raw.get("path"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let mut p = Self::new(
            path,
            vector(raw, "origin")?,
            vector(raw, "u")?,
            vector(raw, "v")?,
        );
        p.aspect = if let Some(v) = raw.get("aspect") {
            crate::presentation::number(v).ok_or_else(|| "Invalid image aspect".to_string())?
        } else {
            1.
        };
        if p.aspect <= 0. {
            p.aspect = 1.;
        }
        p.name = raw
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .into();
        p.opacity = if let Some(v) = raw.get("opacity") {
            crate::presentation::number(v).ok_or_else(|| "Invalid image opacity".to_string())?
        } else {
            1.
        };
        if let Some(v) = raw.get("layer") {
            p.layer = if v.is_null() {
                None
            } else {
                v.as_str().map(str::to_string)
            };
        }
        p.locked = raw
            .get("locked")
            .map(crate::presentation::truth)
            .unwrap_or(false);
        p.native_document = raw.clone();
        p.native_baseline = p.to_json();
        Ok(p)
    }
}
/// Ordered reference-image membership. Pose edits preserve the epoch;
/// structural edits and cloned/restored list owners get a fresh epoch.
#[derive(Debug,Serialize,Deserialize)]
#[serde(transparent)]
pub struct ImagePlaneList {
    planes:Vec<ImagePlane>,
    #[serde(skip,default="new_image_membership_epoch")]
    epoch:std::sync::atomic::AtomicU64,
    #[serde(skip,default="new_image_membership_epoch")]
    publication:std::sync::atomic::AtomicU64,
    #[serde(skip)]
    pose_journal:ImagePoseJournal,
}
/// Fixed bounded runtime journal. Inline storage keeps restricted setter edits
/// allocation-free; it is neither native document data nor history identity.
const IMAGE_POSE_JOURNAL_CAPACITY:usize=64;
#[derive(Debug,Clone,Copy,Default)]struct ImagePoseChange{revision:u64,index:usize}
#[derive(Debug)]struct ImagePoseJournal{entries:[ImagePoseChange;IMAGE_POSE_JOURNAL_CAPACITY],len:usize,next:usize,floor:u64}
impl Default for ImagePoseJournal{fn default()->Self{Self{entries:[ImagePoseChange::default();IMAGE_POSE_JOURNAL_CAPACITY],len:0,next:0,floor:0}}}
impl ImagePoseJournal{
 fn reset(&mut self,revision:u64){self.len=0;self.next=0;self.floor=revision;}
 fn record(&mut self,index:usize,revision:u64){
  if self.len==IMAGE_POSE_JOURNAL_CAPACITY{self.floor=self.entries[self.next].revision;}else{self.len+=1;}
  self.entries[self.next]=ImagePoseChange{revision,index};self.next=(self.next+1)%IMAGE_POSE_JOURNAL_CAPACITY;
 }
}
/// Borrowed bounded delta iterator; the inspection counter exposes actual
/// journal work independently of the number of dirty planes yielded.
pub struct ImagePoseChanges<'a>{entries:&'a[ImagePoseChange],revision:u64,next:usize,examined:usize}
impl ImagePoseChanges<'_>{pub fn records_examined(&self)->usize{self.examined}}
impl Iterator for ImagePoseChanges<'_>{type Item=usize;fn next(&mut self)->Option<usize>{
 while let Some(e)=self.entries.get(self.next){self.next+=1;self.examined+=1;if e.revision>self.revision{return Some(e.index);}}None
}}
fn next_image_membership_epoch()->u64{
    static NEXT:std::sync::atomic::AtomicU64=std::sync::atomic::AtomicU64::new(1);
    NEXT.fetch_add(1,std::sync::atomic::Ordering::Relaxed)
}
fn new_image_membership_epoch()->std::sync::atomic::AtomicU64{std::sync::atomic::AtomicU64::new(next_image_membership_epoch())}
impl From<Vec<ImagePlane>> for ImagePlaneList{fn from(planes:Vec<ImagePlane>)->Self{{let publication=new_image_membership_epoch();let mut pose_journal=ImagePoseJournal::default();pose_journal.reset(publication.load(std::sync::atomic::Ordering::Relaxed));Self{planes,epoch:new_image_membership_epoch(),publication,pose_journal}}}}
impl Default for ImagePlaneList{fn default()->Self{Vec::new().into()}}
impl Clone for ImagePlaneList{fn clone(&self)->Self{self.planes.clone().into()}}
impl PartialEq for ImagePlaneList{fn eq(&self,other:&Self)->bool{self.planes==other.planes}}
impl std::ops::Deref for ImagePlaneList{type Target=[ImagePlane];fn deref(&self)->&Self::Target{&self.planes}}
impl std::ops::Index<usize> for ImagePlaneList{type Output=ImagePlane;fn index(&self,i:usize)->&ImagePlane{&self.planes[i]}}
impl std::ops::IndexMut<usize> for ImagePlaneList{fn index_mut(&mut self,i:usize)->&mut ImagePlane{self.changed();&mut self.planes[i]}}
impl ImagePlaneList{
    pub fn membership_epoch(&self)->u64{self.epoch.load(std::sync::atomic::Ordering::Relaxed)}
    /// Runtime drawable state token; never persisted or compared as model state.
    pub fn publication_revision(&self)->u64{self.publication.load(std::sync::atomic::Ordering::Relaxed)}
    /// A stale owner, structural mutation, or overwritten event requires a cold
    /// rebuild. Events may repeat an index; consumers deduplicate dirty slots.
    pub fn pose_changes_since(&self,epoch:u64,revision:u64)->Option<ImagePoseChanges<'_>>{
        if epoch!=self.membership_epoch()||revision>self.publication_revision()||revision<self.pose_journal.floor{return None;}
        Some(ImagePoseChanges{entries:&self.pose_journal.entries[..self.pose_journal.len],revision,next:0,examined:0})
    }
    fn changed(&mut self){self.epoch.store(next_image_membership_epoch(),std::sync::atomic::Ordering::Relaxed);let revision=next_image_membership_epoch();self.publication.store(revision,std::sync::atomic::Ordering::Relaxed);self.pose_journal.reset(revision);}
    pub fn push(&mut self,p:ImagePlane){self.planes.push(p);self.changed();}
    pub fn remove(&mut self,index:usize)->ImagePlane{let p=self.planes.remove(index);self.changed();p}
    pub fn insert(&mut self,index:usize,p:ImagePlane){self.planes.insert(index,p);self.changed();}
    pub fn reverse(&mut self){if self.planes.len()>1{self.planes.reverse();self.changed();}}
    pub fn retain(&mut self,mut keep:impl FnMut(&ImagePlane)->bool){let n=self.planes.len();self.planes.retain(|p|keep(p));if n!=self.planes.len(){self.changed();}}
    pub fn clear(&mut self){if !self.planes.is_empty(){self.planes.clear();self.changed();}}
    pub fn truncate(&mut self,n:usize){if n<self.planes.len(){self.planes.truncate(n);self.changed();}}
    /// Unrestricted slice edits conservatively invalidate membership.
    pub fn poses_mut(&mut self)->&mut [ImagePlane]{self.changed();&mut self.planes}
    /// Restricted frame mutation cannot change UUID or replace an object,
    /// even when the returned handle is forgotten instead of dropped.
    pub fn pose_mut(&mut self,index:usize)->Option<ImagePoseGuard<'_>>{
        Some(ImagePoseGuard{plane:self.planes.get_mut(index)?,publication:&self.publication,journal:&mut self.pose_journal,index})
    }
    pub fn iter_mut(&mut self)->std::slice::IterMut<'_,ImagePlane>{self.changed();self.planes.iter_mut()}
}
impl<'a> IntoIterator for &'a ImagePlaneList{type Item=&'a ImagePlane;type IntoIter=std::slice::Iter<'a,ImagePlane>;fn into_iter(self)->Self::IntoIter{self.planes.iter()}}
impl<'a> IntoIterator for &'a mut ImagePlaneList{type Item=&'a mut ImagePlane;type IntoIter=std::slice::IterMut<'a,ImagePlane>;fn into_iter(self)->Self::IntoIter{self.changed();self.planes.iter_mut()}}

/// Pose-only mutable access. Identity/whole-object writes require the list's
/// conservatively invalidating mutation APIs.
/// ```compile_fail
/// use ingetrazo_core::image_plane::{ImagePlaneList,ImagePlane};
/// let mut planes=ImagePlaneList::default();
/// let id=planes[0].id;
/// let mut pose=planes.pose_mut(0).unwrap();
/// pose.id=id;
/// ```
/// ```compile_fail
/// use ingetrazo_core::image_plane::{ImagePlaneList,ImagePlane};
/// let mut planes=ImagePlaneList::default();
/// let replacement=planes[0].clone();
/// let mut pose=planes.pose_mut(0).unwrap();
/// *pose=replacement;
/// ```
pub struct ImagePoseGuard<'a>{plane:&'a mut ImagePlane,publication:&'a std::sync::atomic::AtomicU64,journal:&'a mut ImagePoseJournal,index:usize}
impl std::ops::Deref for ImagePoseGuard<'_>{type Target=ImagePlane;fn deref(&self)->&ImagePlane{self.plane}}
impl ImagePoseGuard<'_>{
    pub fn set_origin(&mut self,origin:DVec3){if self.plane.origin!=origin{self.plane.origin=origin;let revision=next_image_membership_epoch();self.publication.store(revision,std::sync::atomic::Ordering::Relaxed);self.journal.record(self.index,revision);}}
    pub fn set_frame(&mut self,[origin,u,v]:[DVec3;3]){if [self.plane.origin,self.plane.u,self.plane.v]!=[origin,u,v]{self.plane.origin=origin;self.plane.u=u;self.plane.v=v;let revision=next_image_membership_epoch();self.publication.store(revision,std::sync::atomic::Ordering::Relaxed);self.journal.record(self.index,revision);}}
}

/// Mirrors original command-held object lifetime without coordinate matching.
#[derive(Debug, Clone, Default)]
pub struct ImageObjects(pub Arc<Mutex<HashMap<uuid::Uuid, ImagePlane>>>);
impl PartialEq for ImageObjects {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}
impl ImageObjects {
    pub fn publish(&self, planes: &[ImagePlane]) {
        let mut objects = self.0.lock().unwrap();
        for p in planes {
            objects.insert(p.id, p.clone());
        }
    }
    /// Update a tool-held detached object's frame without copying live lists.
    pub fn held_frame(&self, image: &ImagePlane) -> [DVec3; 3] {
        let objects = self.0.lock().unwrap();
        let p = objects.get(&image.id).unwrap_or(image);
        [p.origin, p.u, p.v]
    }
    pub fn set_held_frame(&self, image: &ImagePlane, [origin, u, v]: [DVec3; 3]) {
        let mut objects = self.0.lock().unwrap();
        let p = objects.entry(image.id).or_insert_with(|| image.clone());
        p.origin = origin;
        p.u = u;
        p.v = v;
    }
    pub fn set_held_origin(&self, image:&ImagePlane, origin:DVec3) {
        let mut objects=self.0.lock().unwrap();
        objects.entry(image.id).or_insert_with(||image.clone()).origin=origin;
    }
    pub fn metadata_count(&self) -> usize {
        self.0.lock().unwrap().len()
    }
    pub fn retain_ids(&self, ids: &std::collections::HashSet<uuid::Uuid>) -> usize {
        let mut objects = self.0.lock().unwrap();
        let before = objects.len();
        objects.retain(|id, _| ids.contains(id));
        before - objects.len()
    }
    fn held(&self, p: &ImagePlane) -> ImagePlane {
        self.0
            .lock()
            .unwrap()
            .get(&p.id)
            .cloned()
            .unwrap_or_else(|| p.clone())
    }
}
#[derive(Debug, Clone, Default)]
pub struct ImageHistory(pub Option<ImageDelta>);
impl PartialEq for ImageHistory {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}
#[derive(Debug, Clone)]
pub struct ImageDelta {
    pub frame_command: Option<(ImageFrameCommand, bool)>,
    pub add_command: bool,
    /// Original AddImagePlaneCommand refreshes _made_layer on every do.
    pub add_layer_created: Option<Arc<Mutex<bool>>>,
    pub added: Vec<(usize, ImagePlane)>,
    pub removed: Vec<(usize, ImagePlane)>,
    pub changed: Vec<(ImagePlane, ImagePlane)>,
}
impl ImageDelta {
    pub fn between(before: &[ImagePlane], after: &[ImagePlane]) -> Self {
        Self {
            frame_command: None,
            add_layer_created: None,
            add_command: after.iter().any(|a| !before.iter().any(|b| b.id == a.id)),
            added: after
                .iter()
                .enumerate()
                .filter(|(_, p)| !before.iter().any(|b| b.id == p.id))
                .map(|(i, p)| (i, p.clone()))
                .collect(),
            removed: before
                .iter()
                .enumerate()
                .filter(|(_, p)| !after.iter().any(|b| b.id == p.id))
                .map(|(i, p)| (i, p.clone()))
                .collect(),
            changed: before
                .iter()
                .filter_map(|b| {
                    after
                        .iter()
                        .find(|a| a.id == b.id && *a != b)
                        .map(|a| (b.clone(), a.clone()))
                })
                .collect(),
        }
    }
    pub fn inverse(&self) -> Self {
        Self {
            frame_command: self
                .frame_command
                .as_ref()
                .map(|(c, redo)| (c.clone(), !redo)),
            add_command: self.add_command,
            add_layer_created: self.add_layer_created.clone(),
            added: self.removed.clone(),
            removed: self.added.clone(),
            changed: self
                .changed
                .iter()
                .map(|(b, a)| (a.clone(), b.clone()))
                .collect(),
        }
    }
    pub fn restore(&self, current: &[ImagePlane], objects: &ImageObjects) -> Vec<ImagePlane> {
        objects.publish(current);
        let mut target = current.to_vec();
        target.retain(|p| !self.added.iter().any(|(_, a)| a.id == p.id));
        for (i, p) in &self.removed {
            if !target.iter().any(|t| t.id == p.id) {
                target.insert((*i).min(target.len()), objects.held(p));
            }
        }
        if let Some((command, redo)) = &self.frame_command {
            command.apply(&mut target, objects, *redo);
            objects.publish(&target);
            return target;
        }
        for (before, after) in &self.changed {
            let mut p = objects.held(after);
            macro_rules! restore{($($field:ident),*)=>{$(if before.$field!=after.$field{p.$field=before.$field.clone();})*};}
            restore!(origin, u, v, opacity, path, aspect, name, layer, locked);
            objects.0.lock().unwrap().insert(p.id, p.clone());
            if let Some(t) = target.iter_mut().find(|t| t.id == p.id) {
                *t = p;
            }
        }
        objects.publish(&target);
        target
    }
}

/// Source image commands hold object identity, including temporarily detached
/// objects. Relative operations compose on do/undo; Transform captures its
/// complete old frame afresh on every do.
#[derive(Debug, Clone)]
pub struct ImageFrameCommand {
    pub images: Vec<ImagePlane>,
    pub operation: ImageFrameOperation,
}
#[derive(Debug, Clone)]
pub enum ImageFrameOperation {
    Move(DVec3),
    Rotate {
        centre: DVec3,
        axis: DVec3,
        degrees: f64,
    },
    Scale {
        anchor: DVec3,
        factor: DVec3,
        axes: Option<[DVec3; 3]>,
    },
    Transform {
        new: [Option<DVec3>; 3],
        old: Arc<Mutex<Option<[DVec3; 3]>>>,
    },
}
impl ImageFrameCommand {
    pub fn apply(&self, live: &mut [ImagePlane], objects: &ImageObjects, redo: bool) {
        for seed in &self.images {
            let mut image = objects.held(seed);
            match &self.operation {
                ImageFrameOperation::Move(delta) => {
                    image.origin = (image.origin.as_vec3()
                        + if redo {
                            delta.as_vec3()
                        } else {
                            -delta.as_vec3()
                        })
                    .as_dvec3()
                }
                ImageFrameOperation::Rotate {
                    centre,
                    axis,
                    degrees,
                } => {
                    let matrix = ImageFrameMatrix::rotate(
                        *centre,
                        *axis,
                        if redo { *degrees } else { -degrees },
                    );
                    image.origin = matrix.map(image.origin, true);
                    image.u = matrix.map(image.u, false);
                    image.v = matrix.map(image.v, false);
                }
                ImageFrameOperation::Scale {
                    anchor,
                    factor,
                    axes,
                } => {
                    let f = if redo {
                        *factor
                    } else {
                        factor.map(|x| if x.abs() > 1e-12 { 1. / x } else { 1. })
                    };
                    let matrix = ImageFrameMatrix::scale(*anchor, f, *axes);
                    image.origin = matrix.map(image.origin, true);
                    image.u = matrix.map(image.u, false);
                    image.v = matrix.map(image.v, false);
                }
                ImageFrameOperation::Transform { new, old } => {
                    if redo {
                        *old.lock().unwrap() = Some([image.origin, image.u, image.v]);
                        if let Some(x) = new[0] {
                            image.origin = x.as_vec3().as_dvec3();
                        }
                        if let Some(x) = new[1] {
                            image.u = x.as_vec3().as_dvec3();
                        }
                        if let Some(x) = new[2] {
                            image.v = x.as_vec3().as_dvec3();
                        }
                    } else if let Some([o, u, v]) = *old.lock().unwrap() {
                        image.origin = o;
                        image.u = u;
                        image.v = v;
                    }
                }
            }
            objects.0.lock().unwrap().insert(image.id, image.clone());
            if let Some(p) = live.iter_mut().find(|p| p.id == image.id) {
                *p = image;
            }
        }
    }
}
// f32 scalar arithmetic mirrors QVector3D/QMatrix4x4; world placement images
// remain global and are never inverse-fit to a component local frame.
struct ImageFrameMatrix {
    cols: [[f32; 3]; 3],
    translation: [f32; 3],
    kind: u8,
}
impl ImageFrameMatrix {
    fn map(&self, point: DVec3, position: bool) -> DVec3 {
        let p = point.as_vec3().to_array();
        let mut v = [0.; 3];
        for r in 0..3 {
            v[r] = if self.kind == 0 {
                p[r] * self.cols[r][r]
            } else if position && self.kind == 1 && r < 2 {
                p[0] * self.cols[0][r] + p[1] * self.cols[1][r]
            } else if position && self.kind == 1 {
                p[2] * self.cols[2][r]
            } else {
                p[0] * self.cols[0][r] + p[1] * self.cols[1][r] + p[2] * self.cols[2][r]
            };
            if position {
                v[r] += self.translation[r];
            }
        }
        glam::Vec3::from_array(v).as_dvec3()
    }
    fn centered(cols: [[f32; 3]; 3], centre: DVec3, kind: u8) -> Self {
        let c = centre.as_vec3().to_array();
        let mut translation = c;
        for r in 0..3 {
            translation[r] += if kind == 0 {
                -c[r] * cols[r][r]
            } else if kind == 1 && r < 2 {
                -c[0] * cols[0][r] - c[1] * cols[1][r]
            } else if kind == 1 {
                -c[2] * cols[2][r]
            } else {
                -c[0] * cols[0][r] - c[1] * cols[1][r] - c[2] * cols[2][r]
            };
        }
        Self {
            cols,
            translation,
            kind,
        }
    }
    fn rotate(centre: DVec3, axis: DVec3, degrees: f64) -> Self {
        let angle = degrees as f32;
        let (s, c) = match angle {
            90. | -270. => (1., 0.),
            -90. | 270. => (-1., 0.),
            180. | -180. => (0., -1.),
            _ => {
                let a = angle * (std::f64::consts::PI / 180.) as f32;
                (a.sin(), a.cos())
            }
        };
        let a = axis.as_vec3();
        let len = a.length();
        let a = if (len - 1.).abs() <= 1e-5 {
            a
        } else if len.abs() <= 1e-5 {
            glam::Vec3::ZERO
        } else {
            a / len
        };
        let [mut x, mut y, mut z] = a.to_array();
        let (cols, kind) = if x == 0. && y == 0. && z != 0. {
            let s = if z < 0. { -s } else { s };
            ([[c, s, 0.], [-s, c, 0.], [0., 0., 1.]], 1)
        } else if x == 0. && z == 0. && y != 0. {
            let s = if y < 0. { -s } else { s };
            ([[c, 0., -s], [0., 1., 0.], [s, 0., c]], 2)
        } else if y == 0. && z == 0. && x != 0. {
            let s = if x < 0. { -s } else { s };
            ([[1., 0., 0.], [0., c, s], [0., -s, c]], 2)
        } else {
            let len = (x as f64) * (x as f64) + (y as f64) * (y as f64) + (z as f64) * (z as f64);
            if (len - 1.).abs() > 1e-12 * len.min(1.) && len.abs() > 1e-12 {
                let l = len.sqrt();
                x = (x as f64 / l) as f32;
                y = (y as f64 / l) as f32;
                z = (z as f64 / l) as f32;
            }
            let k = 1. - c;
            (
                [
                    [x * x * k + c, y * x * k + z * s, x * z * k - y * s],
                    [x * y * k - z * s, y * y * k + c, y * z * k + x * s],
                    [x * z * k + y * s, y * z * k - x * s, z * z * k + c],
                ],
                2,
            )
        };
        Self::centered(cols, centre, kind)
    }
    fn scale(anchor: DVec3, factor: DVec3, axes: Option<[DVec3; 3]>) -> Self {
        let f = factor.as_vec3().to_array();
        if let Some(axes) = axes {
            let a = axes.map(|a| a.as_vec3().to_array());
            let mut cols = [[0.; 3]; 3];
            for c in 0..3 {
                for r in 0..3 {
                    cols[c][r] = (a[0][r] * f[0]) * a[0][c]
                        + (a[1][r] * f[1]) * a[1][c]
                        + (a[2][r] * f[2]) * a[2][c];
                }
            }
            Self::centered(cols, anchor, 2)
        } else {
            Self::centered([[f[0], 0., 0.], [0., f[1], 0.], [0., 0., f[2]]], anchor, 0)
        }
    }
}

#[derive(Debug,Clone,Copy)]
pub enum ImagePickMode {Inference,Selectable,LockedContext}
impl crate::scene::Scene {
    pub fn image_layer_state(&self,image:&ImagePlane)->(bool,bool){
        let layer=match image.layer.as_deref().filter(|s|!s.is_empty()) {
            Some(name)=>self.layers.iter().find(|l|l.name==name),
            None=>self.layers.iter().find(|l|l.name=="Layer 0"),
        };
        layer.map_or((true,false),|l|(l.visible,l.locked))
    }
    pub fn image_plane_hit(&self,ray:&crate::math::Ray,mode:ImagePickMode)->Option<(usize,f64)>{
        let mut best=None;let mut best_t=f64::INFINITY;
        for (i,image)in self.image_planes.iter().enumerate(){
            let (visible,locked)=self.image_layer_state(image);if !visible{continue;}
            match mode {ImagePickMode::Selectable if image.locked||locked=>continue,
                ImagePickMode::LockedContext if !image.locked||locked=>continue,_=>{}}
            if let Some(t)=image.ray_hit(ray){if t<=best_t{best=Some((i,t));best_t=t;}}
        }best
    }
}

#[cfg(test)]
mod picking_tests {
    use super::*;use crate::{scene::Scene,math::Ray,layers::Layer};
    #[test]
    fn image_modes_preserve_last_tie_locks_and_literal_default_layer() {
        let mut s=Scene::new();s.layers=vec![Layer::new("Images")];
        let a=ImagePlane::new("first",DVec3::ZERO,DVec3::X*2.,DVec3::Y*2.);let mut b=a.clone();b.id=uuid::Uuid::new_v4();b.locked=true;
        s.image_planes=vec![a,b].into();let ray=Ray::new(DVec3::new(1.,1.,10.),-DVec3::Z);
        assert_eq!(s.image_plane_hit(&ray,ImagePickMode::Inference).unwrap().0,1);
        assert_eq!(s.image_plane_hit(&ray,ImagePickMode::Selectable).unwrap().0,0);
        assert_eq!(s.image_plane_hit(&ray,ImagePickMode::LockedContext).unwrap().0,1);
        s.layers[0].locked=true;assert!(s.image_plane_hit(&ray,ImagePickMode::Selectable).is_none());
        assert_eq!(s.image_plane_hit(&ray,ImagePickMode::Inference).unwrap().0,1);
        s.image_planes[0].layer=None;s.image_planes.truncate(1);
        s.layers=vec![Layer::new("Layer0")];s.layers[0].visible=false;
        assert!(s.image_plane_hit(&ray,ImagePickMode::Selectable).is_some());
        s.layers[0].name="Layer 0".into();assert!(s.image_plane_hit(&ray,ImagePickMode::Inference).is_none());
    }
    #[test]
    fn source_degenerate_and_sheared_projection_is_not_replaced_by_polygon_pick() {
        let mut p=ImagePlane::new("scan",DVec3::ZERO,DVec3::ZERO,DVec3::ZERO);
        let ray=Ray::new(DVec3::new(100.,100.,10.),-DVec3::Z);assert_eq!(p.ray_hit(&ray),Some(10.));
        p.u=DVec3::X*2.;p.v=DVec3::new(1.,2.,0.);
        assert!(p.ray_hit(&Ray::new(DVec3::new(2.25,0.5,10.),-DVec3::Z)).is_none());
    }
}

impl ImagePlane {
    /// Source QMatrix4x4 frame pose without allocating a document or history.
    pub fn rotated_frame(&self, centre: DVec3, axis: DVec3, degrees: f64) -> [DVec3; 3] {
        ImageRotation::new(centre, axis, degrees).frame(self)
    }
}
/// One source Qt-f32 matrix shared by all images in a Rotate preview.
pub struct ImageRotation(ImageFrameMatrix);
impl ImageRotation {
    pub fn new(centre: DVec3, axis: DVec3, degrees: f64) -> Self {
        Self(ImageFrameMatrix::rotate(centre, axis, degrees))
    }
    pub fn frame(&self, image: &ImagePlane) -> [DVec3; 3] {
        [
            self.0.map(image.origin, true),
            self.0.map(image.u, false),
            self.0.map(image.v, false),
        ]
    }
}

#[cfg(test)]mod publication_revision_tests {
 use super::*;
 #[test]fn pose_publication_is_immediate_noop_stable_and_membership_independent(){
  let mut p=ImagePlaneList::from(vec![ImagePlane::new("x",DVec3::ZERO,DVec3::X,DVec3::Y)]);
  let membership=p.membership_epoch();let initial=p.publication_revision();
  p.pose_mut(0).unwrap().set_origin(DVec3::ZERO);assert_eq!(p.publication_revision(),initial);
  {let mut handle=p.pose_mut(0).unwrap();handle.set_origin(DVec3::Z);std::mem::forget(handle);}
  assert_ne!(p.publication_revision(),initial);assert_eq!(p.membership_epoch(),membership);
  let changed=p.publication_revision();p.pose_mut(0).unwrap().set_frame([DVec3::Z,DVec3::X,DVec3::Y]);assert_eq!(p.publication_revision(),changed);
  let clone=p.clone();assert_eq!(clone,p);assert_ne!(clone.publication_revision(),changed);
  let encoded=serde_json::to_value(&p).unwrap();assert!(encoded.is_array());let decoded:ImagePlaneList=serde_json::from_value(encoded).unwrap();assert_eq!(decoded,p);assert_ne!(decoded.publication_revision(),changed);
  p[0].opacity=0.5;assert_ne!(p.publication_revision(),changed);
 }
}

#[cfg(test)]mod pose_journal_tests{
 use super::*;
 fn planes()->ImagePlaneList{vec![ImagePlane::new("missing",DVec3::ZERO,DVec3::X,DVec3::Y);2].into()}
 #[test]fn restricted_changes_are_immediate_and_survive_forgotten_guard(){
  let mut p=planes();let epoch=p.membership_epoch();let initial=p.publication_revision();
  p.pose_mut(0).unwrap().set_origin(DVec3::ZERO);assert_eq!(p.publication_revision(),initial);assert_eq!(p.pose_changes_since(epoch,initial).unwrap().count(),0);
  let mut pose=p.pose_mut(1).unwrap();pose.set_origin(DVec3::Z);std::mem::forget(pose);
  assert_eq!(p.membership_epoch(),epoch);assert_eq!(p.pose_changes_since(epoch,initial).unwrap().collect::<Vec<_>>(),vec![1]);
  let changed=p.publication_revision();p.pose_mut(0).unwrap().set_frame([DVec3::X,DVec3::Y,DVec3::Z]);
  assert_eq!(p.pose_changes_since(epoch,changed).unwrap().collect::<Vec<_>>(),vec![0]);
 }
 #[test]fn overflow_and_every_structural_owner_change_refuse_stale_readers(){
  let mut p=planes();let epoch=p.membership_epoch();let initial=p.publication_revision();
  for i in 1..=IMAGE_POSE_JOURNAL_CAPACITY{p.pose_mut(0).unwrap().set_origin(DVec3::new(i as f64,0.,0.));}
  assert_eq!(p.pose_changes_since(epoch,initial).unwrap().count(),IMAGE_POSE_JOURNAL_CAPACITY);
  let before_overwrite=p.publication_revision();p.pose_mut(1).unwrap().set_origin(DVec3::Y);
  assert!(p.pose_changes_since(epoch,initial).is_none());assert_eq!(p.pose_changes_since(epoch,before_overwrite).unwrap().collect::<Vec<_>>(),vec![1]);
  let changed=p.publication_revision();p.reverse();assert!(p.pose_changes_since(epoch,changed).is_none());
  let epoch=p.membership_epoch();let changed=p.publication_revision();p[0].id=uuid::Uuid::new_v4();assert!(p.pose_changes_since(epoch,changed).is_none());
  let epoch=p.membership_epoch();let changed=p.publication_revision();let clone=p.clone();assert!(clone.pose_changes_since(epoch,changed).is_none());
  let data=serde_json::to_value(&p).unwrap();assert!(data.is_array());let decoded:ImagePlaneList=serde_json::from_value(data).unwrap();assert_eq!(decoded,p);assert!(decoded.pose_changes_since(epoch,changed).is_none());
  assert!(p.pose_changes_since(p.membership_epoch(),u64::MAX).is_none());
 }
}
