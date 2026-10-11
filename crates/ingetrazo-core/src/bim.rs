// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! BIM / IFC tagging layer and quantity takeoff (area, volume).

use serde::{Deserialize, Serialize};

pub const IFC_CLASSES: &[&str] = &[
    "IfcWall",
    "IfcSlab",
    "IfcColumn",
    "IfcBeam",
    "IfcFooting",
    "IfcRoof",
    "IfcStair",
    "IfcRamp",
    "IfcDoor",
    "IfcWindow",
    "IfcRailing",
    "IfcCovering",
    "IfcPile",
    "IfcMember",
    "IfcSpace",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BimTag {
    pub id: usize,
    pub class_name: String,
    pub name: String,
}

impl BimTag {
    pub fn new(id: usize, class_name: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id,
            class_name: class_name.into(),
            name: name.into(),
        }
    }
}

use crate::{groups::Group, mesh::Mesh, scene::Scene};
use glam::{DMat4, DVec3, Vec3};
use std::{
    borrow::Cow,
    collections::{BTreeMap, HashMap},
};

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Quantity {
    pub kind: &'static str,
    pub name: &'static str,
    pub value: f64,
}
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Quantities {
    pub set: String,
    pub entries: Vec<Quantity>,
    pub metrado: Option<f64>,
    pub unit: &'static str,
}
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum ObjectKey {
    Faces(Option<i64>),
    Group(usize),
}
pub struct Object<'a> {
    pub key: ObjectKey,
    pub class: String,
    pub name: String,
    pub mesh: Cow<'a, Mesh>,
    pub faces: Vec<usize>,
}
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Takeoff {
    pub key: ObjectKey,
    pub class: String,
    pub name: String,
    pub area: f64,
    pub volume: Option<f64>,
    pub quantities: Quantities,
}

pub fn next_object_id(scene: &Scene) -> Result<i64, String> {
    scene
        .mesh
        .faces
        .iter()
        .filter_map(|f| {
            f.attrs
                .get("ifc")
                .and_then(|v| v.get("id"))
                .and_then(|v| v.as_i64())
        })
        .fold(0, i64::max)
        .checked_add(1)
        .ok_or_else(|| "BIM object identity exhausted".into())
}
fn qt_len(v: Vec3) -> f32 {
    let m = v.abs().max_element();
    if m == 0. {
        return 0.;
    }
    let n = v / m;
    m * (n.x * n.x + n.y * n.y + n.z * n.z).sqrt()
}
fn newell(mesh: &Mesh, ring: &[usize]) -> Vec3 {
    let mut n = DVec3::ZERO;
    for i in 0..ring.len() {
        let a = mesh.vertices[ring[i]].position.as_vec3().as_dvec3();
        let b = mesh.vertices[ring[(i + 1) % ring.len()]]
            .position
            .as_vec3()
            .as_dvec3();
        n += DVec3::new(
            (a.y - b.y) * (a.z + b.z),
            (a.z - b.z) * (a.x + b.x),
            (a.x - b.x) * (a.y + b.y),
        );
    }
    n.as_vec3()
}
fn area(mesh: &Mesh, id: usize) -> f64 {
    qt_len(newell(mesh, &mesh.faces[id].vertices)) as f64 * 0.5
}
fn net_area(mesh: &Mesh, id: usize) -> f64 {
    let mut a = area(mesh, id);
    for ring in &mesh.faces[id].holes {
        let mut n = Vec3::ZERO;
        for i in 0..ring.len() {
            let a = mesh.vertices[ring[i]].position.as_vec3().as_dvec3();
            let b = mesh.vertices[ring[(i + 1) % ring.len()]]
                .position
                .as_vec3()
                .as_dvec3();
            n += DVec3::new(
                (a.y - b.y) * (a.z + b.z),
                (a.z - b.z) * (a.x + b.x),
                (a.x - b.x) * (a.y + b.y),
            )
            .as_vec3();
        }
        a -= qt_len(n) as f64 * 0.5;
    }
    a.max(0.)
}
fn normal(mesh: &Mesh, id: usize) -> Vec3 {
    let n = newell(mesh, &mesh.faces[id].vertices);
    let len = qt_len(n);
    if len < 1e-9 {
        Vec3::Z
    } else {
        n / len
    }
}
fn validate(mesh: &Mesh, faces: &[usize]) -> Result<(), String> {
    for &id in faces {
        let f = mesh.faces.get(id).ok_or("Invalid BIM face identity")?;
        for ring in std::iter::once(&f.vertices).chain(f.holes.iter()) {
            if ring.len() < 3 {
                return Err("BIM face has fewer than three corners".into());
            }
            for &i in ring {
                let p = mesh
                    .vertices
                    .get(i)
                    .ok_or("Invalid BIM vertex identity")?
                    .position;
                if !p.is_finite() || !p.as_vec3().is_finite() {
                    return Err("Nonfinite BIM coordinates".into());
                }
            }
        }
    }
    Ok(())
}
/// Closure is tested on this object's complete ring edges, including holes.
/// No bbox volume and no volume claim for an open or nonmanifold face set.
pub fn face_set_volume(mesh: &Mesh, faces: &[usize]) -> Option<f64> {
    if faces.is_empty() {
        return None;
    }
    let mut counts = HashMap::<(usize, usize), usize>::new();
    for &id in faces {
        let f = &mesh.faces[id];
        for ring in std::iter::once(&f.vertices).chain(f.holes.iter()) {
            for i in 0..ring.len() {
                let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
                *counts.entry((a.min(b), a.max(b))).or_default() += 1;
            }
        }
    }
    if counts.values().any(|&n| n != 2) {
        return None;
    }
    let mut total = 0.;
    for &id in faces {
        for (a, b, c) in mesh.triangulate_face(id) {
            let (a, b, c) = (a.as_vec3(), b.as_vec3(), c.as_vec3());
            let cross = b.cross(c);
            let dot = a.x * cross.x + a.y * cross.y + a.z * cross.z;
            total += dot as f64 / 6.;
        }
    }
    Some(total.abs())
}
fn measure(
    class: &str,
    mesh: &Mesh,
    faces: &[usize],
) -> Result<(Quantities, f64, Option<f64>), String> {
    validate(mesh, faces)?;
    let unit = match class {
        "IfcColumn" | "IfcBeam" | "IfcFooting" => "m3",
        "IfcPile" | "IfcMember" | "IfcRailing" => "m",
        "IfcDoor" | "IfcWindow" => "und",
        _ => "m2",
    };
    let set = if IFC_CLASSES.contains(&class) {
        format!("Qto_{}BaseQuantities", class.strip_prefix("Ifc").unwrap())
    } else {
        "BaseQuantities".into()
    };
    let mut q = Quantities {
        set,
        entries: vec![],
        metrado: None,
        unit,
    };
    if faces.is_empty() {
        return Ok((q, 0., None));
    }
    let volume = face_set_volume(mesh, faces);
    if volume.is_some_and(|v| !v.is_finite())
        || faces
            .iter()
            .any(|&id| !area(mesh, id).is_finite() || !net_area(mesh, id).is_finite())
    {
        return Err("BIM quantities exceed finite source arithmetic".into());
    }
    let closed = volume.is_some();
    let total = faces.iter().map(|&id| area(mesh, id)).sum::<f64>();
    let mut lo = DVec3::splat(f64::INFINITY);
    let mut hi = DVec3::splat(f64::NEG_INFINITY);
    let mut big = faces[0];
    for &id in faces {
        if area(mesh, id) > area(mesh, big) {
            big = id;
        }
        for ring in std::iter::once(&mesh.faces[id].vertices).chain(mesh.faces[id].holes.iter()) {
            for &i in ring {
                let p = mesh.vertices[i].position.as_vec3().as_dvec3();
                lo = lo.min(p);
                hi = hi.max(p);
            }
        }
    }
    let d = hi - lo;
    let gross = area(mesh, big);
    let net = net_area(mesh, big);
    let mut add = |kind, name, value: Option<f64>| {
        if let Some(v) = value.filter(|v| *v > 1e-9) {
            q.entries.push(Quantity {
                kind,
                name,
                value: v,
            });
        }
    };
    q.metrado = match class {
        "IfcWall" => {
            add("length", "Height", Some(d.z));
            if d.z > 1e-9 {
                add("length", "Length", Some(gross / d.z));
            }
            if net > 1e-9 {
                add("length", "Width", volume.map(|v| v / net));
            }
            add("area", "GrossSideArea", Some(gross));
            add("area", "NetSideArea", Some(net));
            add("volume", "GrossVolume", volume);
            Some(net)
        }
        "IfcSlab" => {
            let ring = &mesh.faces[big].vertices;
            let perimeter = (0..ring.len())
                .map(|i| {
                    qt_len(
                        mesh.vertices[ring[i]].position.as_vec3()
                            - mesh.vertices[ring[(i + 1) % ring.len()]].position.as_vec3(),
                    ) as f64
                })
                .sum();
            add("length", "Perimeter", Some(perimeter));
            if net > 1e-9 {
                add("length", "Width", volume.map(|v| v / net));
            }
            add("area", "GrossArea", Some(gross));
            add("area", "NetArea", Some(net));
            add("volume", "GrossVolume", volume);
            Some(net)
        }
        "IfcRoof" | "IfcStair" | "IfcRamp" => {
            let up = faces
                .iter()
                .filter(|&&id| {
                    let z = normal(mesh, id).z;
                    (if closed { z } else { z.abs() }) > 0.2
                })
                .map(|&id| net_area(mesh, id))
                .sum();
            add("length", "Length", Some(d.x.max(d.y)));
            add("area", "GrossArea", Some(up));
            add("volume", "GrossVolume", volume);
            Some(up)
        }
        "IfcColumn" | "IfcBeam" => {
            let length = if class == "IfcColumn" {
                d.z
            } else {
                d.max_element()
            };
            add("length", "Length", Some(length));
            if length > 1e-9 {
                add("area", "CrossSectionArea", volume.map(|v| v / length));
            }
            add("volume", "GrossVolume", volume);
            volume
        }
        "IfcFooting" => {
            add("length", "Height", Some(d.z));
            add("area", "GrossSurfaceArea", Some(total));
            add("volume", "GrossVolume", volume);
            volume
        }
        "IfcPile" | "IfcMember" | "IfcRailing" => {
            let length = match class {
                "IfcPile" => d.z,
                "IfcRailing" => d.x.max(d.y),
                _ => d.max_element(),
            };
            add("length", "Length", Some(length));
            add("volume", "GrossVolume", volume);
            Some(length)
        }
        "IfcCovering" => {
            let net = faces.iter().map(|&id| net_area(mesh, id)).sum();
            add("area", "GrossArea", Some(total));
            add("area", "NetArea", Some(net));
            Some(net)
        }
        "IfcDoor" | "IfcWindow" => {
            let u = Vec3::Z.cross(normal(mesh, big));
            let length = qt_len(u);
            let (width, height) = if length < 1e-6 {
                let mut lo = DVec3::splat(f64::INFINITY);
                let mut hi = DVec3::splat(f64::NEG_INFINITY);
                for ring in
                    std::iter::once(&mesh.faces[big].vertices).chain(mesh.faces[big].holes.iter())
                {
                    for &id in ring {
                        let p = mesh.vertices[id].position.as_vec3().as_dvec3();
                        lo = lo.min(p);
                        hi = hi.max(p);
                    }
                }
                let d = hi - lo;
                (d.x.max(d.y), d.z)
            } else {
                let u = if (length - 1.).abs() < 1e-5 {
                    u
                } else {
                    u / length
                };
                let mut us = vec![];
                let mut zs = vec![];
                for &id in &mesh.faces[big].vertices {
                    let p = mesh.vertices[id].position.as_vec3();
                    us.push((p.x * u.x + p.y * u.y + p.z * u.z) as f64);
                    zs.push(p.z as f64);
                }
                let extent = |v: &[f64]| {
                    v.iter().copied().fold(f64::NEG_INFINITY, f64::max)
                        - v.iter().copied().fold(f64::INFINITY, f64::min)
                };
                (extent(&us), extent(&zs))
            };
            add("length", "Width", Some(width));
            add("length", "Height", Some(height));
            add("area", "Area", Some(net));
            Some(1.)
        }
        "IfcSpace" => {
            let floor = faces
                .iter()
                .filter(|&&id| normal(mesh, id).z.abs() > 0.7)
                .map(|&id| net_area(mesh, id))
                .fold(0., f64::max);
            add("length", "Height", Some(d.z));
            add("area", "NetFloorArea", Some(floor));
            add("volume", "GrossVolume", volume);
            Some(floor)
        }
        _ => {
            add("area", "GrossArea", Some(total));
            add("volume", "GrossVolume", volume);
            Some(total)
        }
    };
    if q.entries.iter().any(|e| !e.value.is_finite()) || q.metrado.is_some_and(|v| !v.is_finite()) {
        return Err("BIM quantities exceed finite source arithmetic".into());
    }
    q.metrado = q.metrado.filter(|v| *v > 1e-9);
    Ok((q, total, volume))
}
fn world_mesh(group: &Group) -> Result<Mesh, String> {
    fn append(g: &Group, parent: DMat4, out: &mut Mesh) -> Result<(), String> {
        let world = parent * g.transform;
        if !world.is_finite() {
            return Err("Nonfinite BIM placement".into());
        }
        validate(&g.mesh, &(0..g.mesh.faces.len()).collect::<Vec<_>>())?;
        let mut ids = Vec::with_capacity(g.mesh.vertices.len());
        for v in &g.mesh.vertices {
            let p = world.transform_point3(v.position).as_vec3().as_dvec3();
            if !p.is_finite() {
                return Err("Nonfinite transformed BIM vertex".into());
            }
            ids.push(out.add_vertex(p));
        }
        for f in &g.mesh.faces {
            let outer = f.vertices.iter().map(|&i| ids[i]).collect();
            let holes = f
                .holes
                .iter()
                .map(|h| h.iter().map(|&i| ids[i]).collect())
                .collect();
            out.add_face(outer, holes)
                .ok_or("Degenerate transformed BIM face")?;
        }
        for child in &g.children {
            append(child, world, out)?;
        }
        Ok(())
    }
    let mut out = Mesh::new();
    append(group, DMat4::IDENTITY, &mut out)?;
    Ok(out)
}
pub fn collect_objects(scene: &Scene) -> Result<Vec<Object<'_>>, String> {
    collect_objects_from(&scene.mesh,&scene.groups)
}
/// Borrow the original loose-face domain and active placement list independently.
pub fn collect_objects_from<'a>(loose:&'a Mesh,groups:&'a [Group])->Result<Vec<Object<'a>>,String>{
    let mut by_id = BTreeMap::<Option<i64>, (&serde_json::Value, Vec<usize>)>::new();
    for (id, f) in loose.faces.iter().enumerate() {
        if let Some(tag) = f
            .attrs
            .get("ifc")
            .filter(|v| v.as_object().is_some_and(|m| !m.is_empty()))
        {
            let key = match tag.get("id") {
                None | Some(serde_json::Value::Null) => None,
                Some(n) => Some(n.as_i64().ok_or("BIM object id requires integer")?),
            };
            by_id.entry(key).or_insert_with(|| (tag, vec![])).1.push(id);
        }
    }
    let mut out = Vec::new();
    let mut ordered: Vec<_> = by_id.into_iter().collect();
    ordered.sort_by_key(|(id, (_, faces))| (id.unwrap_or(0), faces[0]));
    for (id, (tag, faces)) in ordered {
        out.push(Object {
            key: ObjectKey::Faces(id),
            class: tag
                .get("class")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .into(),
            name: tag
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .into(),
            mesh: Cow::Borrowed(loose),
            faces,
        });
    }
    for (i, g) in groups.iter().enumerate() {
        if let Some(tag) = g
            .ifc_tag
            .as_ref()
            .filter(|v| v.as_object().is_some_and(|m| !m.is_empty()))
        {
            let mesh = if g.transform == DMat4::IDENTITY && g.children.is_empty() {
                Cow::Borrowed(&g.mesh)
            } else {
                Cow::Owned(world_mesh(g)?)
            };
            let faces = (0..mesh.faces.len()).collect();
            out.push(Object {
                key: ObjectKey::Group(i),
                class: tag
                    .get("class")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .into(),
                name: tag
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .into(),
                mesh,
                faces,
            });
        }
    }
    for o in &out {
        validate(&o.mesh, &o.faces)?;
    }
    Ok(out)
}
pub fn class_quantities(class: &str, mesh: &Mesh, faces: &[usize]) -> Result<Quantities, String> {
    Ok(measure(class, mesh, faces)?.0)
}
/// One measurement supplies the table, IFC shape classification and quantities.
pub fn object_takeoff(o: &Object<'_>) -> Result<Takeoff, String> {
    let (quantities, area, volume) = measure(&o.class, &o.mesh, &o.faces)?;
    Ok(Takeoff {
        key: o.key.clone(),
        class: o.class.clone(),
        name: o.name.clone(),
        area,
        volume,
        quantities,
    })
}
pub fn takeoff(scene: &Scene) -> Result<Vec<Takeoff>, String> {
    collect_objects(scene)?.iter().map(object_takeoff).collect()
}

pub fn takeoff_from(loose:&Mesh,groups:&[Group])->Result<Vec<Takeoff>,String>{collect_objects_from(loose,groups)?.iter().map(object_takeoff).collect()}

pub fn quantities_csv(rows: &[Takeoff]) -> String {
    let mut s = "class,name,metrado,unit,area_m2,volume_m3\n".to_owned();
    for r in rows {
        let met = r
            .quantities
            .metrado
            .map(|v| format!("{v:.4}"))
            .unwrap_or_default();
        let vol = r.volume.map(|v| format!("{v:.4}")).unwrap_or_default();
        s.push_str(&format!(
            "{},\"{}\",{met},{},{:.4},{vol}\n",
            r.class,
            r.name.replace('"', "'"),
            r.quantities.unit,
            r.area
        ));
    }
    s
}

/// Command-owned direct faces only. Topology-rebuilding draw plans stamp their
/// declarations before coverage reconstruction instead of calling this hook.
pub(crate) fn json_truthy(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Null => false,
        serde_json::Value::Bool(v) => *v,
        serde_json::Value::Number(v) => v.as_f64().is_some_and(|n| n != 0.),
        serde_json::Value::String(v) => !v.is_empty(),
        serde_json::Value::Array(v) => !v.is_empty(),
        serde_json::Value::Object(v) => !v.is_empty(),
    }
}
pub fn next_draw_object_id(mesh: &Mesh) -> Result<i64, String> {
    let mut top = 0;
    for face in &mesh.faces {
        let Some(tag) = face.attrs.get("ifc").filter(|t| json_truthy(t)) else {
            continue;
        };
        let object = tag
            .as_object()
            .ok_or("Existing BIM metadata must be an object before active drawing")?;
        let Some(id) = object.get("id") else { continue };
        let id = if let Some(v) = id.as_i64() {
            Some(v)
        } else if let Some(v) = id.as_bool() {
            Some(i64::from(v))
        } else if id.as_u64().is_some() {
            return Err("BIM object identity exceeds supported integer range".into());
        } else {
            None
        };
        if let Some(id) = id {
            top = top.max(id)
        }
    }
    top.checked_add(1)
        .ok_or_else(|| "BIM object identity exhausted".into())
}
pub fn stamp_drawn_faces(
    mesh: &mut Mesh,
    owned: &[usize],
    active: Option<&serde_json::Value>,
) -> Result<(), String> {
    stamp_drawn_faces_with_id(mesh,owned,active,None)
}
/// Explicit source allocation domain; callers cannot infer authored faces from topology churn.
pub fn stamp_drawn_faces_with_id(mesh:&mut Mesh,owned:&[usize],active:Option<&serde_json::Value>,source_id:Option<&Result<i64,String>>)->Result<(),String>{
    let Some(active) = active else { return Ok(()) };
    if !json_truthy(active) {
        return Ok(());
    }
    let object = active
        .as_object()
        .ok_or("Active BIM tag must be an object")?;
    if object.is_empty() {
        return Ok(());
    }
    if owned.is_empty() {
        return Ok(());
    }
    for &id in owned {
        if id >= mesh.faces.len() {
            return Err("Drawn BIM face command no longer live".into());
        }
    }
    let id = match source_id {Some(id)=>id.clone()?,None=>next_draw_object_id(mesh)?};
    let mut tag = active.as_object().unwrap().clone();
    tag.insert("id".into(), id.into());
    let tag = serde_json::Value::Object(tag);
    for &id in owned {
        let face = &mut mesh.faces[id];
        if !face.attrs.get("ifc").is_some_and(json_truthy) {
            face.attrs.insert("ifc".into(), tag.clone());
        }
    }
    Ok(())
}

#[cfg(test)]
mod drawn_truthiness_tests {
    use super::*;
    use serde_json::json;
    fn face() -> Mesh {
        let mut m = Mesh::new();
        let ids = [DVec3::ZERO, DVec3::X, DVec3::Y].map(|p| m.add_vertex(p));
        m.add_face(ids.to_vec(), vec![]);
        m
    }
    #[test]
    fn empty_json_ifc_values_follow_python_truthiness() {
        for value in [
            json!(null),
            json!(false),
            json!(0),
            json!(0.0),
            json!(""),
            json!([]),
            json!({}),
        ] {
            assert!(!json_truthy(&value));
            let mut mesh = face();
            mesh.faces[0].attrs.insert("ifc".into(), value);
            stamp_drawn_faces(
                &mut mesh,
                &[0],
                Some(&json!({"class":"IfcWall","name":"Wall"})),
            )
            .unwrap();
            assert_eq!(mesh.faces[0].attrs["ifc"]["id"], 1);
        }
    }
    #[test]
    fn opaque_truthy_nonobject_refuses_without_overwriting() {
        for value in [json!(true), json!(1), json!("opaque"), json!([17])] {
            assert!(json_truthy(&value));
            let mut mesh = face();
            mesh.faces[0].attrs.insert("ifc".into(), value);
            let before = mesh.clone();
            assert!(stamp_drawn_faces(&mut mesh, &[0], Some(&json!({"class":"IfcWall"}))).is_err());
            assert_eq!(mesh, before);
        }
    }
    #[test]
    fn inherited_truthy_object_and_python_boolean_identity_are_preserved() {
        let mut mesh = face();
        let tag = json!({"id":true,"vendor":71});
        mesh.faces[0].attrs.insert("ifc".into(), tag.clone());
        stamp_drawn_faces(&mut mesh, &[0], Some(&json!({"class":"IfcWall"}))).unwrap();
        assert_eq!(mesh.faces[0].attrs["ifc"], tag);
        assert_eq!(next_draw_object_id(&mesh).unwrap(), 2);
    }
}

/// Runtime-only direct BIM metadata command; stable targets survive edit lifting.
/// Source performs these mutations outside History. Native retains its existing
/// explicit Undo extension without replacing an unrelated mesh/context.
#[derive(Debug,Clone,PartialEq)]
pub struct MetadataHistory { pub targets:Vec<MetadataTarget> }
#[derive(Debug,Clone,PartialEq)]
pub struct MetadataTarget {
    pub context:Vec<uuid::Uuid>,
    pub group:Option<uuid::Uuid>,
    pub vertices:Vec<uuid::Uuid>,
    pub value:Option<serde_json::Value>,
    pub expected:Option<serde_json::Value>,
}
