// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! Saved views / Scenes: named snapshots of camera and layer visibility.

use glam::DVec3;
use serde::{Deserialize, Serialize};

use crate::camera::OrbitCamera;
use crate::layers::Layer;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedView {
    /// Original commands retain view objects by identity; never written to IGZ.
    #[serde(skip, default = "uuid::Uuid::new_v4")]
    pub history_id: uuid::Uuid,
    #[serde(default)]
    pub native_document: serde_json::Value,
    #[serde(default)]
    pub native_baseline: serde_json::Value,
    #[serde(default)] pub two_point: bool,
    #[serde(default)] pub hidden_objects: Option<Vec<String>>,
    #[serde(default)] pub style: Option<serde_json::Value>,
    #[serde(default)] pub section: Option<serde_json::Value>,
    #[serde(default)] pub georef: Option<serde_json::Value>,
    #[serde(default)] pub shadows: Option<serde_json::Value>,
    #[serde(default)] pub hidden_shown: Option<serde_json::Value>,
    pub name: String,
    #[serde(default)]
    pub target: DVec3,
    #[serde(default = "default_distance")]
    pub distance: f64,
    #[serde(default = "default_yaw")]
    pub yaw: f64,
    #[serde(default = "default_pitch")]
    pub pitch: f64,
    #[serde(default = "default_fov")]
    pub fov_deg: f64,
    #[serde(default = "default_true", alias = "perspective")]
    pub perspective: bool,
    #[serde(default, alias = "parallel")]
    pub parallel: bool,
    #[serde(default)]
    pub hidden_layers: Vec<String>,
}

fn default_distance() -> f64 { 20.0 }
fn default_yaw() -> f64 { (-45.0_f64).to_radians() }
fn default_pitch() -> f64 { 30.0_f64.to_radians() }
fn default_fov() -> f64 { 45.0 }
fn default_true() -> bool { true }

impl SavedView {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            history_id:uuid::Uuid::new_v4(),
            two_point:false,hidden_objects:None,style:None,section:None,georef:None,shadows:None,hidden_shown:None,
            native_document: serde_json::Value::Null,
            native_baseline: serde_json::Value::Null,
            name: name.into(),
            target: DVec3::ZERO,
            distance: default_distance(),
            yaw: default_yaw(),
            pitch: default_pitch(),
            fov_deg: default_fov(),
            perspective: true,
            parallel: false,
            hidden_layers: Vec::new(),
        }
    }

    pub fn capture(name: impl Into<String>, camera: &OrbitCamera, layers: &[Layer]) -> Self {
        let hidden = layers.iter()
            .filter(|l| !l.visible)
            .map(|l| l.name.clone())
            .collect();

        Self {
            history_id:uuid::Uuid::new_v4(),
            two_point:camera.two_point,hidden_objects:None,style:None,section:None,georef:None,shadows:None,hidden_shown:None,
            native_document: serde_json::Value::Null,
            native_baseline: serde_json::Value::Null,
            name: name.into(),
            target: camera.target,
            distance: camera.distance,
            yaw: camera.yaw,
            pitch: camera.pitch,
            fov_deg: camera.fov_deg,
            perspective: camera.perspective,
            parallel: !camera.perspective,
            hidden_layers: hidden,
        }
    }

    /// Preserve raw identity/extension metadata when refreshing a view.
    pub fn capture_scene(name:impl Into<String>,scene:&crate::scene::Scene,camera:&OrbitCamera)->Self {
        let mut view=Self::capture(name,camera,&scene.layers);view.two_point=camera.two_point;view.style=Some(serde_json::to_value(&scene.display_style).unwrap());view.shadows=Some(serde_json::to_value(&scene.shadows).unwrap());
        let identities = group_identities(&scene.groups);
        view.hidden_objects = Some(identities.into_iter().filter_map(|(id, _, hidden)| hidden.then_some(id)).collect());
        let active=scene.section_planes.iter().find(|s|s.active).map(|s|s.native_uid());
        view.section=Some(serde_json::json!({"active":active,"planes_shown":scene.show_section_planes,"cuts_shown":scene.show_section_cuts}));
        view.georef=Some(serde_json::json!({"terrain":scene.terrain.as_ref().map(|t|t.visible).unwrap_or(false)}));view
    }
    pub fn apply_scene(&self,scene:&mut crate::scene::Scene,camera:&mut OrbitCamera) {
        self.apply(camera,&mut scene.layers);
        if let Some(style)=&self.style{scene.display_style=crate::presentation::DocumentStyle::from_json(style);scene.show_section_fill=scene.display_style.section_fill;}
        if let Some(shadows)=&self.shadows{scene.shadows=crate::presentation::ShadowSettings::from_json(shadows);}
        if let Some(ids)=&self.hidden_objects {
            let winners = group_identities(&scene.groups).into_iter().map(|(_, ordinal, _)| ordinal).collect::<std::collections::HashSet<_>>();
            let hidden = ids.iter().map(String::as_str).collect::<std::collections::HashSet<_>>();
            fn apply(groups: &mut [crate::groups::Group], winners: &std::collections::HashSet<usize>, hidden: &std::collections::HashSet<&str>, next: &mut usize) {
                for group in groups {
                    let ordinal = *next; *next += 1;
                    if winners.contains(&ordinal) { group.hidden = hidden.contains(group.native_uid().as_str()); }
                    apply(&mut group.children, winners, hidden, next);
                }
            }
            apply(&mut scene.groups, &winners, &hidden, &mut 0);
        }
        if let Some(section)=&self.section {
            let active=section.get("active");
            for plane in &mut scene.section_planes{let id=plane.native_uid();plane.active=active==Some(&id);}
            scene.show_section_planes=section.get("planes_shown").and_then(|v|v.as_bool()).unwrap_or(true);
            scene.show_section_cuts=section.get("cuts_shown").and_then(|v|v.as_bool()).unwrap_or(true);
        }
        if let (Some(georef),Some(terrain))=(&self.georef,&mut scene.terrain){if let Some(visible)=georef.get("terrain").and_then(|v|v.as_bool()){terrain.visible=visible;}}
        scene.camera=camera.clone();scene.bump_view_version();
    }

    pub fn apply(&self, camera: &mut OrbitCamera, layers: &mut [Layer]) {
        camera.two_point=self.two_point;
        camera.target = self.target;
        camera.distance = self.distance;
        camera.yaw = self.yaw;
        camera.pitch = self.pitch;
        camera.fov_deg = self.fov_deg;
        camera.perspective = if self.parallel { false } else { self.perspective };

        for layer in layers.iter_mut() {
            layer.visible = !self.hidden_layers.contains(&layer.name);
        }
    }
}

impl PartialEq for SavedView {fn eq(&self,other:&Self)->bool {self.native_document==other.native_document && self.native_baseline==other.native_baseline && self.two_point==other.two_point && self.hidden_objects==other.hidden_objects && self.style==other.style && self.section==other.section && self.georef==other.georef && self.shadows==other.shadows && self.hidden_shown==other.hidden_shown && self.name==other.name && self.target==other.target && self.distance==other.distance && self.yaw==other.yaw && self.pitch==other.pitch && self.fov_deg==other.fov_deg && self.perspective==other.perspective && self.parallel==other.parallel && self.hidden_layers==other.hidden_layers}}

/// A single model command's view ownership, distinct from ScenesPanel actions.
#[derive(Debug,Clone,PartialEq)]
pub struct ViewMembershipDelta {pub added:Vec<(usize,SavedView)>,pub removed:Vec<(usize,SavedView)>}
impl ViewMembershipDelta {
 pub fn between(before:&[SavedView],after:&[SavedView])->Self{let before_ids=before.iter().map(|v|v.history_id).collect::<std::collections::HashSet<_>>();let after_ids=after.iter().map(|v|v.history_id).collect::<std::collections::HashSet<_>>();Self{added:after.iter().enumerate().filter(|(_,v)|!before_ids.contains(&v.history_id)).map(|(i,v)|(i,v.clone())).collect(),removed:before.iter().enumerate().filter(|(_,v)|!after_ids.contains(&v.history_id)).map(|(i,v)|(i,v.clone())).collect()}}
 pub fn inverse(&self)->Self{Self{added:self.removed.clone(),removed:self.added.clone()}}
 pub fn restore(&self,current:&[SavedView])->Vec<SavedView>{let added=self.added.iter().map(|(_,v)|v.history_id).collect::<std::collections::HashSet<_>>();let mut result=current.iter().filter(|v|!added.contains(&v.history_id)).cloned().collect::<Vec<_>>();let mut live=result.iter().map(|v|v.history_id).collect::<std::collections::HashSet<_>>();for (_,v) in &self.removed {if live.insert(v.history_id){result.push(v.clone());}}result}
 pub fn refresh(&mut self,view:&SavedView){for (_,owned) in self.added.iter_mut().chain(&mut self.removed){if owned.history_id==view.history_id{*owned=view.clone();}}}
}

/// Sealed-empty and unsealed history are semantically equal Scene snapshots;
/// the seal bit only prevents later non-command edits being attributed to a command.
#[derive(Debug,Clone,Default)]
pub struct ViewHistory(pub Option<ViewMembershipDelta>);
impl PartialEq for ViewHistory {fn eq(&self,other:&Self)->bool{match(&self.0,&other.0){(None,Some(d))|(Some(d),None)=>d.added.is_empty()&&d.removed.is_empty(),(a,b)=>a==b}}}

/// Python dictionary order and last-placement resolution, using literal disk IDs.
fn group_identities(groups: &[crate::groups::Group]) -> Vec<(String, usize, bool)> {
    fn visit(groups: &[crate::groups::Group], next: &mut usize, slots: &mut std::collections::HashMap<String, usize>, result: &mut Vec<(String, usize, bool)>) {
        for g in groups {
            let ordinal = *next; *next += 1;
            let id = g.native_uid();
            if let Some(&slot) = slots.get(&id) { result[slot].1 = ordinal; result[slot].2 = g.hidden; }
            else { slots.insert(id.clone(), result.len()); result.push((id, ordinal, g.hidden)); }
            visit(&g.children, next, slots, result);
        }
    }
    let mut result = Vec::new();
    visit(groups, &mut 0, &mut std::collections::HashMap::new(), &mut result);
    result
}
