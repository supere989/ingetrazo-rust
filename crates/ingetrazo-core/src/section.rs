// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! Section planes — cutting planes slicing through 3D models.
//!
//! A section plane consists of a point and unit normal vector.
//! The active cut clips the display to peer inside closed geometries.

use glam::DVec3;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Calculate the next conventional letter symbol (A, B, C, ..., AA, AB...).
pub fn next_symbol(existing: &[SectionPlane]) -> String {
    let taken: std::collections::HashSet<String> = existing
        .iter()
        .map(|p| p.symbol.trim().to_uppercase())
        .collect();

    let mut i = 0;
    loop {
        let mut n = i;
        let mut label = String::new();
        loop {
            let rem = n % 26;
            label.insert(0, (b'A' + rem as u8) as char);
            n = n / 26 - 1;
            if n < 0 {
                break;
            }
        }
        if !taken.contains(&label) {
            return label;
        }
        i += 1;
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SectionPlane {
    #[serde(default)]
    pub native_document: serde_json::Value,
    #[serde(default)]
    pub native_baseline: serde_json::Value,
    #[serde(default = "Uuid::new_v4")]
    pub id: Uuid,
    pub point: DVec3,
    pub normal: DVec3,
    pub name: String,
    pub symbol: String,
    pub active: bool,
}

impl SectionPlane {
    pub fn new(point: DVec3, normal: DVec3, name: impl Into<String>, symbol: impl Into<String>) -> Self {
        let n = if normal.length_squared() < 1e-12 {
            DVec3::Z
        } else {
            normal.normalize()
        };
        Self {
            native_document: serde_json::Value::Null,
            native_baseline: serde_json::Value::Null,
            id: Uuid::new_v4(),
            point,
            normal: n,
            name: name.into(),
            symbol: symbol.into(),
            active: true,
        }
    }

    pub fn native_uid(&self)->serde_json::Value {
        let current=serde_json::json!(self.id.as_u128() as u64);
        if self.native_baseline.get("uid").map(|v|v==&current).unwrap_or(true){if let Some(uid)=self.native_document.get("uid"){return uid.clone();}}
        current
    }

    /// Reverse normal: cut shows the opposite side.
    pub fn flip(&mut self) {
        self.normal = -self.normal;
    }

    /// Signed distance to point `p`. Positive if on the clipped/hidden side.
    pub fn signed_distance(&self, p: DVec3) -> f64 {
        self.normal.dot(p - self.point)
    }

    /// Check if point is clipped by this plane (i.e. on the normal side).
    pub fn is_clipped(&self, p: DVec3) -> bool {
        self.signed_distance(p) > 0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_section_symbols() {
        let planes = vec![
            SectionPlane::new(DVec3::ZERO, DVec3::Z, "Section 1", "A"),
            SectionPlane::new(DVec3::ZERO, DVec3::Z, "Section 2", "B"),
        ];
        assert_eq!(next_symbol(&planes), "C");
    }

    #[test]
    fn test_section_clipping() {
        let mut sp = SectionPlane::new(DVec3::new(0.0, 0.0, 5.0), DVec3::Z, "Cut", "A");
        assert!(sp.is_clipped(DVec3::new(0.0, 0.0, 6.0)));
        assert!(!sp.is_clipped(DVec3::new(0.0, 0.0, 4.0)));
        sp.flip();
        assert!(!sp.is_clipped(DVec3::new(0.0, 0.0, 6.0)));
        assert!(sp.is_clipped(DVec3::new(0.0, 0.0, 4.0)));
    }
}

impl crate::scene::Scene{
    pub fn active_section(&self)->Option<&SectionPlane>{if self.show_section_cuts{self.section_planes.iter().find(|plane|plane.active)}else{None}}
    pub fn section_point_visible(&self,point:DVec3)->bool{self.active_section().is_none_or(|plane|plane.signed_distance(point)<=1e-7)}
    pub fn section_segment(&self,a:DVec3,b:DVec3)->Option<(DVec3,DVec3)>{let Some(plane)=self.active_section()else{return Some((a,b));};let da=plane.signed_distance(a);let db=plane.signed_distance(b);if da>1e-7&&db>1e-7{return None;}if da>1e-7||db>1e-7{let p=a+(b-a)*(da/(da-db));if da>1e-7{Some((p,b))}else{Some((a,p))}}else{Some((a,b))}}
}

impl crate::scene::Scene{pub fn section_frame(&self,index:usize)->Option<[DVec3;4]>{let plane=self.section_planes.get(index)?;let reference=if plane.normal.z.abs()<0.9{DVec3::Z}else{DVec3::X};let u=reference.cross(plane.normal).normalize();let v=plane.normal.cross(u).normalize();let mut points=self.mesh.vertices.iter().map(|vertex|vertex.position).collect::<Vec<_>>();fn groups(groups:&[crate::groups::Group],parent:glam::DMat4,points:&mut Vec<DVec3>){for group in groups{let transform=parent*group.transform;points.extend(group.mesh.vertices.iter().map(|vertex|transform.transform_point3(vertex.position)));children(&group.children,transform,points);}}fn children(children:&[crate::groups::Group],parent:glam::DMat4,points:&mut Vec<DVec3>){groups(children,parent,points)}groups(&self.groups,glam::DMat4::IDENTITY,&mut points);let(mut lo_u,mut hi_u,mut lo_v,mut hi_v)=(-1.5,1.5,-1.5,1.5);if !points.is_empty(){lo_u=f64::INFINITY;hi_u=f64::NEG_INFINITY;lo_v=f64::INFINITY;hi_v=f64::NEG_INFINITY;for p in points{let p=p-plane.point;lo_u=lo_u.min(p.dot(u));hi_u=hi_u.max(p.dot(u));lo_v=lo_v.min(p.dot(v));hi_v=hi_v.max(p.dot(v));}let mu=((hi_u-lo_u)*0.1).max(0.2);let mv=((hi_v-lo_v)*0.1).max(0.2);lo_u-=mu;hi_u+=mu;lo_v-=mv;hi_v+=mv;}Some([(lo_u,lo_v),(hi_u,lo_v),(hi_u,hi_v),(lo_u,hi_v)].map(|(x,y)|plane.point+u*x+v*y))}}
