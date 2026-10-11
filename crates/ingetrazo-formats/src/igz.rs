// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! Native IngeTrazo document format (.igz).
//!
//! Format 1: Plain JSON
//! Format 2: ZIP container with document.json and textures/

use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use glam::{DMat4, DVec3};
use serde::{Deserialize, Serialize};
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

use ingetrazo_core::dimensions::Dimension;
use ingetrazo_core::groups::Group;
use ingetrazo_core::guides::Guide;
use ingetrazo_core::layers::Layer;
use ingetrazo_core::materials::Material;
use ingetrazo_core::mesh::Mesh;
use ingetrazo_core::scene::Scene;
use ingetrazo_core::units::LengthUnit;

mod assets;

pub const ZIP_MAGIC: &[u8] = b"PK\x03\x04";

#[derive(thiserror::Error, Debug)]
pub enum IgzError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("ZIP error: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("Invalid document structure: {0}")]
    InvalidDocument(String),
}

#[derive(Serialize, Deserialize)]
pub struct IgzEnvelope {
    pub igz_format: u32,
    pub app_version: String,
    pub scene: serde_json::Value,
}

fn mesh_to_json(mesh: &Mesh) -> serde_json::Value {
    let mut edges_json = Vec::new();
    for e in &mesh.edges {
        let p0 = mesh.vertices[e.v0].position;
        let p1 = mesh.vertices[e.v1].position;
        let mut obj = serde_json::json!({
            "a": [p0.x, p0.y, p0.z],
            "b": [p1.x, p1.y, p1.z],
        });
        if e.soft {
            obj["soft"] = serde_json::Value::Bool(true);
        }
        if e.hidden {
            obj["hidden"] = serde_json::Value::Bool(true);
        }
        if let Some(ref l) = e.layer {
            obj["layer"] = serde_json::Value::String(l.clone());
        }
        for (key, value) in &e.attrs {
            if !["a", "b", "soft", "hidden"].contains(&key.as_str()) && !(key == "layer" && e.layer.is_some()) {
                obj[key] = value.clone();
            }
        }
        if let Some(curve)=e.curve_id {obj["curve"]=serde_json::json!(curve);}
        edges_json.push(obj);
    }

    let mut faces_json = Vec::new();
    for f in &mesh.faces {
        let verts: Vec<Vec<f64>> = f.vertices.iter().map(|&v| {
            let p = mesh.vertices[v].position;
            vec![p.x, p.y, p.z]
        }).collect();

        let holes: Vec<Vec<Vec<f64>>> = f.holes.iter().map(|h| {
            h.iter().map(|&v| {
                let p = mesh.vertices[v].position;
                vec![p.x, p.y, p.z]
            }).collect()
        }).collect();

        let mut obj = serde_json::json!({
            "vertices": verts,
        });
        if !holes.is_empty() {
            obj["holes"] = serde_json::json!(holes);
        }
        if let Some(ref l) = f.layer {
            obj["layer"] = serde_json::Value::String(l.clone());
        }
        for (k, v) in &f.attrs {
            if k != "layer" || f.layer.is_none() { obj[k] = v.clone(); }
        }
        faces_json.push(obj);
    }

    serde_json::json!({
        "edges": edges_json,
        "faces": faces_json,
    })
}

fn json_to_mesh(val: &serde_json::Value) -> Mesh {
    let mut mesh = Mesh::new();
    if let Some(edges) = val.get("edges").and_then(|e| e.as_array()) {
        for e in edges {
            if let (Some(a), Some(b)) = (e.get("a").and_then(|v| v.as_array()), e.get("b").and_then(|v| v.as_array())) {
                if a.len() >= 3 && b.len() >= 3 {
                    let pa = DVec3::new(
                        a[0].as_f64().unwrap_or(0.0),
                        a[1].as_f64().unwrap_or(0.0),
                        a[2].as_f64().unwrap_or(0.0),
                    );
                    let pb = DVec3::new(
                        b[0].as_f64().unwrap_or(0.0),
                        b[1].as_f64().unwrap_or(0.0),
                        b[2].as_f64().unwrap_or(0.0),
                    );
                    let va = mesh.add_vertex(pa);
                    let vb = mesh.add_vertex(pb);
                    if let Some(edge_id) = mesh.add_edge(va, vb) {
                        if let Some(obj) = e.as_object() {
                            for (key,value) in obj {
                                if !["a","b","soft","hidden"].contains(&key.as_str()) && !(key == "layer" && value.is_string()) {
                                    mesh.edges[edge_id].attrs.insert(key.clone(),value.clone());
                                }
                            }
                        }
                        mesh.edges[edge_id].curve_id=e.get("curve").and_then(|v|v.as_u64()).and_then(|i|usize::try_from(i).ok());
                        if e.get("soft").and_then(|s| s.as_bool()).unwrap_or(false) {
                            mesh.edges[edge_id].soft = true;
                        }
                        if e.get("hidden").and_then(|h| h.as_bool()).unwrap_or(false) {
                            mesh.edges[edge_id].hidden = true;
                        }
                        if let Some(layer) = e.get("layer").and_then(|l| l.as_str()) {
                            mesh.edges[edge_id].layer = Some(layer.to_string());
                        }
                    }
                }
            }
        }
    }

    if let Some(faces) = val.get("faces").and_then(|f| f.as_array()) {
        for f in faces {
            if let Some(verts_arr) = f.get("vertices").and_then(|v| v.as_array()) {
                let mut loop_verts = Vec::new();
                for pt in verts_arr {
                    if let Some(coords) = pt.as_array() {
                        if coords.len() >= 3 {
                            let p = DVec3::new(
                                coords[0].as_f64().unwrap_or(0.0),
                                coords[1].as_f64().unwrap_or(0.0),
                                coords[2].as_f64().unwrap_or(0.0),
                            );
                            loop_verts.push(mesh.add_vertex(p));
                        }
                    }
                }

                let mut holes_verts = Vec::new();
                if let Some(holes_arr) = f.get("holes").and_then(|h| h.as_array()) {
                    for hole in holes_arr {
                        if let Some(h_pts) = hole.as_array() {
                            let mut h_loop = Vec::new();
                            for pt in h_pts {
                                if let Some(coords) = pt.as_array() {
                                    if coords.len() >= 3 {
                                        let p = DVec3::new(
                                            coords[0].as_f64().unwrap_or(0.0),
                                            coords[1].as_f64().unwrap_or(0.0),
                                            coords[2].as_f64().unwrap_or(0.0),
                                        );
                                        h_loop.push(mesh.add_vertex(p));
                                    }
                                }
                            }
                            if h_loop.len() >= 3 {
                                holes_verts.push(h_loop);
                            }
                        }
                    }
                }

                if loop_verts.len() >= 3 {
                    if let Some(face_id) = mesh.add_face(loop_verts, holes_verts) {
                        if let Some(layer) = f.get("layer").and_then(|l| l.as_str()) {
                            mesh.faces[face_id].layer = Some(layer.to_string());
                        }
                        if let Some(obj) = f.as_object() {
                            for (k, v) in obj {
                                if k != "vertices" && k != "holes" && !(k == "layer" && v.is_string()) {
                                    mesh.faces[face_id].attrs.insert(k.clone(), v.clone());
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    mesh
}

/// Overlay changed modeled fields only. Unknown and unmodeled native data
/// remain owned by the document, rather than discarded during deserialization.
fn overlay(original: &serde_json::Value, baseline: &serde_json::Value,
           current: serde_json::Value) -> serde_json::Value {
    if !original.is_object() { return current; }
    let mut result = original.clone();
    for (key, value) in current.as_object().unwrap() {
        if baseline.get(key) != Some(value) {
            // Geometry metadata is carried by MeshEdge/Face identity. Overlaying
            // by array position would transfer deleted entities' attrs to new ones.
            result[key] = if key == "edges" || key == "faces" {
                value.clone()
            } else { merge_changed(result.get(key), baseline.get(key), value) };

        }
    }
    result
}

fn merge_changed(original: Option<&serde_json::Value>, baseline: Option<&serde_json::Value>,
                 current: &serde_json::Value) -> serde_json::Value {
    if baseline == Some(current) {
        return original.unwrap_or(current).clone();
    }
    if let (Some(o), Some(b)) = (original, baseline) {
        if current.is_object() && o.is_object() && b.is_object() {
            return overlay(o, b, current.clone());
        }
        if let (Some(oa), Some(ba), Some(ca)) = (o.as_array(), b.as_array(), current.as_array()) {
            if ca.iter().all(|v| v.get("name").and_then(|v| v.as_str()).is_some())
                && ba.iter().all(|v| v.get("name").and_then(|v| v.as_str()).is_some())
                && oa.iter().all(|v| v.get("name").and_then(|v| v.as_str()).is_some()) {
                return serde_json::json!(ca.iter().map(|c| {
                    let name = c.get("name");
                    let o = oa.iter().find(|v| v.get("name") == name);
                    let b = ba.iter().find(|v| v.get("name") == name);
                    merge_changed(o,b,c)
                }).collect::<Vec<_>>());
            }
            if oa.len() == ba.len() && ba.len() == ca.len() {
                return serde_json::json!(ca.iter().enumerate().map(|(i,c)|
                    merge_changed(Some(&oa[i]),Some(&ba[i]),c)).collect::<Vec<_>>());
            }
        }
    }
    current.clone()
}

fn group_model(g: &Group) -> serde_json::Value {
    let mut obj = mesh_to_json(&g.mesh);
    obj["name"] = serde_json::json!(g.name);
    obj["uid"] = serde_json::json!(g.id.simple().to_string());
    obj["component"] = serde_json::json!(g.component_definition.is_some());
    // Internal comparison identity, removed before writing native groups.
    obj["_component_definition"] = serde_json::json!(g.component_definition);
    obj["_mesh_share"] = serde_json::json!(g.mesh_share);
    obj["xform"] = serde_json::json!(g.transform.to_cols_array());
    obj["layer"] = serde_json::json!(g.layer);
    obj["ifc"] = serde_json::json!(g.ifc_tag);
    obj["hidden"] = serde_json::json!(g.hidden);
    obj["billboard"] = serde_json::json!(g.billboard);
    obj["children"] = serde_json::json!(g.children.iter().map(group_model).collect::<Vec<_>>());
    obj
}

fn group_to_json(g: &Group, protos: &mut Vec<serde_json::Value>, shared: &mut std::collections::HashMap<(uuid::Uuid,bool),usize>, mesh_shares: &std::collections::HashMap<uuid::Uuid,Mesh>) -> serde_json::Value {
    let model = group_model(g);
    let mut obj = overlay(&g.native_document, &g.native_baseline, model.clone());
    // Legacy documents predate names/UIDs. Once the group list is published,
    // retain the identities assigned on load, rather than regenerate on reopen.
    if obj.get("name").and_then(|v|v.as_str()).is_none_or(str::is_empty) { obj["name"] = model["name"].clone(); }
    if obj.get("uid").and_then(|v|v.as_str()).is_none_or(str::is_empty) { obj["uid"] = model["uid"].clone(); }
    // Rust meshes are independent copies. Editing an imported instance makes a
    // new definition; never mutate a retained prototype used by other instances.
    let geometry_changed = ["edges", "faces"].iter()
        .any(|k| g.native_baseline.get(*k) != model.get(*k));
    // Only published geometry retains shared identity. A direct placement edit
    // must not overwrite or silently reuse another placement's prototype.
    let published_share = g.mesh_share.filter(|id| mesh_shares.get(id)
        .is_some_and(|mesh| mesh_to_json(mesh) == mesh_to_json(&g.mesh)));
    if let Some(definition)=published_share.or(g.component_definition) {
        let key=(definition,published_share.is_some());
        let idx=if let Some(idx)=shared.get(&key) {*idx} else {
            let original_idx=g.native_document.get("proto").and_then(|v|v.as_u64()).map(|i|i as usize);
            let idx=if !geometry_changed && original_idx.is_some()
                && !shared.values().any(|idx|Some(*idx)==original_idx) {original_idx.unwrap()} else {
                let original_mesh=original_idx.and_then(|i|protos.get(i)).unwrap_or(&g.native_document);
                let baseline_mesh=serde_json::json!({"edges":g.native_baseline.get("edges"),"faces":g.native_baseline.get("faces")});
                let mesh=overlay(original_mesh,&baseline_mesh,mesh_to_json(&g.mesh));
                let idx=protos.len();protos.push(mesh);idx
            };
            shared.insert(key,idx);idx
        };
        obj["proto"]=serde_json::json!(idx);
        // overlay already reconciles the raw source component flag with the
        // typed current value and baseline. A mesh-share is not itself a user
        // component definition; do not erase unchanged imported metadata.

        obj.as_object_mut().unwrap().remove("edges");
        obj.as_object_mut().unwrap().remove("faces");
    } else if g.native_document.get("proto").is_some() && (geometry_changed
        || g.native_baseline.get("component").and_then(|v|v.as_bool())==Some(true)
        || g.native_baseline.get("_mesh_share").is_some_and(|v| !v.is_null())) {
        obj["proto"] = serde_json::json!(protos.len());
        let original_mesh = g.native_document.get("proto").and_then(|v| v.as_u64())
            .and_then(|i| protos.get(i as usize)).unwrap_or(&g.native_document);
        let baseline_mesh = serde_json::json!({
            "edges": g.native_baseline.get("edges"), "faces": g.native_baseline.get("faces")
        });
        let preserved_mesh = overlay(original_mesh, &baseline_mesh, mesh_to_json(&g.mesh));
        protos.push(preserved_mesh);
        obj.as_object_mut().unwrap().remove("edges");
        obj.as_object_mut().unwrap().remove("faces");
    } else if g.native_document.is_null()
        || (g.native_document.get("proto").is_none()
            && g.native_baseline.get("xform") != model.get("xform")) {
        // Python only applies xform to prototype instances.
        obj["proto"] = serde_json::json!(protos.len());
        let baseline_mesh = serde_json::json!({
            "edges": g.native_baseline.get("edges"), "faces": g.native_baseline.get("faces")
        });
        protos.push(overlay(&g.native_document, &baseline_mesh, mesh_to_json(&g.mesh)));
        obj.as_object_mut().unwrap().remove("edges");
        obj.as_object_mut().unwrap().remove("faces");
    }
    // The source reader only resolves proto geometry with a placement matrix.
    // Creating a component from an imported baked group must therefore replace
    // its preserved null xform; ordinary baked groups remain matrix-free.
    if obj.get("proto").is_some_and(|v|v.is_number()) && obj["xform"].is_null() {
        obj["xform"] = model["xform"].clone();
    }
    let children = g.children.iter().map(|c| group_to_json(c, protos, shared, mesh_shares)).collect::<Vec<_>>();
    if !g.children.is_empty() || g.native_baseline.get("children") != model.get("children") {
        obj["children"] = serde_json::json!(children);
    }
    obj.as_object_mut().unwrap().remove("_component_definition");
    obj.as_object_mut().unwrap().remove("_mesh_share");
    obj
}

fn material_model(mat: &Material) -> serde_json::Value {
    serde_json::json!({
        "name": mat.name, "color": mat.color, "opacity": mat.opacity,
        "roughness": mat.roughness,
        "texture": {"path": mat.texture_path, "embed": mat.texture_embed}
    })
}

fn point_model(point: &ingetrazo_core::georef::GeoPoint) -> serde_json::Value {
    serde_json::json!({"position":point.position,"name":point.name,"desc":point.desc})
}

fn path_model(path: &ingetrazo_core::georef::GeoPath) -> serde_json::Value {
    use ingetrazo_core::georef::SurfaceFillMode;
    let surface = path.surface.as_ref().map(|mode| match mode {
        SurfaceFillMode::Flat => "flat", SurfaceFillMode::Draped => "draped"
    });
    serde_json::json!({"points":path.points,"closed":path.closed,"name":path.name,"surface":surface})
}

fn scale_bar_width(scale_n: f64, segments: usize) -> f64 {
    if segments == 0 || !scale_n.is_finite() || scale_n <= 0. { return 0.; }
    let raw_m=(100. / segments as f64)*scale_n/1000.;
    let nice=[0.1,0.2,0.25,0.5,1.,2.,2.5,5.,10.,20.,25.,50.,100.,200.,250.,500.,1000.,2000.,5000.];
    let best=nice.into_iter().filter(|n|*n<=raw_m).last().unwrap_or(0.1);
    best*1000./scale_n*segments as f64
}

fn sheet_item_model(item: &ingetrazo_core::composition::SheetItem) -> (uuid::Uuid, &'static str, serde_json::Value) {
    use ingetrazo_core::composition::SheetItem;
    match item {
        SheetItem::ViewFrame(f) => (f.id,"frames",serde_json::json!({
            "x_mm":f.rect.x,"y_mm":f.rect.y,"w_mm":f.rect.w,"h_mm":f.rect.h,
            "scale_n":f.scale_n,"view_key":f.view_key,"style":match f.style.as_str() {"technical"=>"vectorial","wireframe"=>"lineas",s=>s},
            "show_title":f.show_title,"title_text":f.title_text,"border":f.border,
            "perspective":f.perspective,"rot_deg":f.rot_deg,"annotations":f.annotations,
            "paper_bg":f.paper_bg,"cut_fill":f.cut_fill,"cut_fill_color":f.cut_fill_color,"cut_hatch_mm":f.cut_hatch_mm,
            "pen_cut_mm":f.pen_cut_mm,"pen_profile_mm":f.pen_profile_mm,"pen_edge_mm":f.pen_edge_mm,"profiles":f.profiles,"hidden_lines":f.hidden_lines,
            "cam_target":f.cam_target,"cam_yaw":f.cam_yaw,"cam_pitch":f.cam_pitch,"cam_distance":f.cam_distance,"cam_fov":f.cam_fov,
            "section_marks":f.section_marks,"uid":f.id.simple().to_string()
        })),
        SheetItem::TitleBlock(f) => (f.id,"cajetin",serde_json::json!({
            "x_mm":f.rect.x,"y_mm":f.rect.y,"w_mm":f.rect.w,"h_mm":f.rect.h,
            "proyecto":f.project_name,"autor":f.author,"fecha":f.date,
            "lamina":f.sheet_number,"escala":f.scale_text,"drawing_title":f.drawing_title
        })),
        SheetItem::Text(f) => (f.id,"texts",serde_json::json!({
            "x_mm":f.pos.0,"y_mm":f.pos.1,"text":f.text,"size_pt":f.font_size_pt,"bold":f.bold
        })),
        SheetItem::NorthArrow(f) => (f.id,"nortes",serde_json::json!({
            "x_mm":f.pos.0,"y_mm":f.pos.1,"size_mm":f.size_mm,"angle_deg":f.rot_deg
        })),
        SheetItem::Shape(f) => (f.id,"shapes",serde_json::to_value(&f.shape).expect("finite sheet shape")),
        SheetItem::GraphicScale(f) => (f.id,"scalebars",serde_json::json!({
            "x_mm":f.pos.0,"y_mm":f.pos.1,"scale_n":f.scale_n,"segments":f.divisions
        }))
    }
}

fn sheet_model(sheet: &ingetrazo_core::composition::Sheet) -> serde_json::Value {
    use ingetrazo_core::composition::Orientation;
    let mut model = serde_json::json!({
        "name":sheet.name,"paper":sheet.paper_size,
        "landscape":sheet.orientation==Orientation::Landscape,"margin_mm":sheet.margin_mm,
        "frames":[],"texts":[],"nortes":[],"scalebars":[],"shapes":[],"cajetin":null
    });
    for item in &sheet.items {
        let (id,key,current)=sheet_item_model(item);
        let empty=serde_json::Value::Null;
        let original=sheet.native_items.get(&id).unwrap_or(&empty);
        let baseline=sheet.native_item_baselines.get(&id).unwrap_or(&empty);
        let mut value=overlay(original,baseline,current.clone());
        if key=="cajetin" {
            if let Some(rows)=original.get("campos").and_then(|v| v.as_array()) {
                let mut rows=rows.clone();
                for (label,field) in [("PROYECTO","proyecto"),("AUTOR","autor"),("FECHA","fecha"),("ESCALA","escala"),("LÁMINA","lamina"),("TÍTULO","drawing_title")] {
                    if baseline.get(field)!=current.get(field) {
                        if let Some(row)=rows.iter_mut().find(|r| r.get(0).and_then(|v| v.as_str()).map(|s|s.trim().to_uppercase()==label).unwrap_or(false)) {
                            row[1]=current[field].clone();
                        } else { rows.push(serde_json::json!([label,current[field]])); }
                    }
                }
                value["campos"]=serde_json::json!(rows);
            } else {
                value["campos"]=serde_json::json!([
                    ["PROYECTO",current["proyecto"]],["AUTOR",current["autor"]],
                    ["FECHA",current["fecha"]],["ESCALA",current["escala"]],
                    ["LÁMINA",current["lamina"]],["TÍTULO",current["drawing_title"]]
                ]);
            }
            model[key]=value;
        } else { model[key].as_array_mut().unwrap().push(value); }
    }
    model
}

/// Project only pinned source paper context fields; never copy unrelated archive payloads.
/// The caller supplies the fixed dataclass field/default policy, not a whole Scene.
pub fn paper_context_sheet_record(sheet:&ingetrazo_core::composition::Sheet,classes:&serde_json::Value)->Result<serde_json::Value,String>{
 use ingetrazo_core::composition::SheetItem;
 use serde_json::{Map,Value};
 struct Count(usize);
 impl std::io::Write for Count {
  fn write(&mut self,data:&[u8])->std::io::Result<usize>{self.0=self.0.checked_sub(data.len()).ok_or_else(||std::io::Error::other("Paper context record budget exceeded"))?;Ok(data.len())}
  fn flush(&mut self)->std::io::Result<()>{Ok(())}
 }
 let mut count=Count(4*1024*1024);
 fn project(raw:&Value,defaults:&Value,count:&mut Count)->Result<Value,String>{
  let mut fields=Map::new();
  for(key,default)in defaults.as_object().ok_or("Invalid paper context policy")?{
   let value=raw.get(key).unwrap_or(default);
   serde_json::to_writer(&mut *count,key).map_err(|_|"Paper context record budget exceeded")?;
   serde_json::to_writer(&mut *count,value).map_err(|_|"Paper context record budget exceeded")?;
   if let Some(value)=raw.get(key){fields.insert(key.clone(),value.clone());}
  }
  Ok(Value::Object(fields))
 }
 if sheet.items.len()>10000{return Err("Paper context item budget exceeded".into());}
 serde_json::to_writer(&mut count,&(&sheet.name,&sheet.paper_size,sheet.margin_mm)).map_err(|_|"Paper context record budget exceeded")?;
 let mut model=serde_json::json!({"name":sheet.name,"paper":sheet.paper_size,"landscape":sheet.orientation==ingetrazo_core::composition::Orientation::Landscape,"margin_mm":sheet.margin_mm,"frames":[],"texts":[],"nortes":[],"scalebars":[],"shapes":[],"cajetin":null});
 serde_json::to_writer(&mut count,&model).map_err(|_|"Paper context record budget exceeded")?;
 let empty=Value::Null;
 for item in &sheet.items{
  // Typed string admission precedes current-item materialization.
  serde_json::to_writer(&mut count,item).map_err(|_|"Paper context record budget exceeded")?;
  let(id,key,current)=sheet_item_model(item);let defaults=classes.get(key).ok_or("Invalid paper context category")?;
  let original=project(sheet.native_items.get(&id).unwrap_or(&empty),defaults,&mut count)?;
  let baseline=project(sheet.native_item_baselines.get(&id).unwrap_or(&empty),defaults,&mut count)?;
  let mut value=overlay(&original,&baseline,current.clone());
  if let SheetItem::TitleBlock(_)=item{
   // Match the codec's operative row updates without carrying opaque outer metadata.
   if let Some(rows)=original.get("campos").and_then(Value::as_array){
    let mut rows=rows.clone();
    for(label,field)in [("PROYECTO","proyecto"),("AUTOR","autor"),("FECHA","fecha"),("ESCALA","escala"),("LÁMINA","lamina"),("TÍTULO","drawing_title")]{
     if baseline.get(field)!=current.get(field){
      if let Some(row)=rows.iter_mut().find(|r|r.get(0).and_then(Value::as_str).map(|s|s.trim().to_uppercase()==label).unwrap_or(false)){row[1]=current[field].clone();}
      else{rows.push(serde_json::json!([label,current[field]]));}
     }
    }
    value["campos"]=Value::Array(rows);
   }else{value["campos"]=serde_json::json!([["PROYECTO",current["proyecto"]],["AUTOR",current["autor"]],["FECHA",current["fecha"]],["ESCALA",current["escala"]],["LÁMINA",current["lamina"]],["TÍTULO",current["drawing_title"]]]);}
   model[key]=value;
  }else{model[key].as_array_mut().unwrap().push(value);}
 }
 for(kind,defaults)in classes.as_object().ok_or("Invalid paper context policy")?{
  if matches!(kind.as_str(),"frames"|"texts"|"nortes"|"scalebars"|"shapes"|"cajetin"){continue;}
  let mut items=vec![];
  if let Some(raw)=sheet.native_document.get(kind).and_then(Value::as_array){
   if raw.len()>10000{return Err("Paper context item budget exceeded".into());}
   for record in raw.iter().filter(|v|v.is_object()){items.try_reserve(1).map_err(|_|"Paper context item allocation failed")?;items.push(project(record,defaults,&mut count)?);}
  }
  model[kind]=Value::Array(items);
 }
 for(key,fields)in [("border",&["on","mm","color","radius_mm","style"][..]),("guides",&["v","h"][..])]{
  if let Some(raw)=sheet.native_document.get(key){let mut selected=Map::new();for field in fields{if let Some(value)=raw.get(field){serde_json::to_writer(&mut count,value).map_err(|_|"Paper context record budget exceeded")?;selected.insert((*field).into(),value.clone());}}model[key]=Value::Object(selected);}
 }
 model["border"]=serde_json::to_value(sheet.effective_source_border()).map_err(|_|"Invalid paper border")?;
 Ok(model)
}

fn sheet_to_json(sheet: &ingetrazo_core::composition::Sheet) -> serde_json::Value {
    let current=sheet_model(sheet);
    let mut result=overlay(&sheet.native_document,&sheet.native_baseline,current.clone());
    // Modeled items already carry their own retained metadata; don't merge
    // stale array positions after deleting, moving or replacing an item.
    for key in ["frames","texts","nortes","scalebars","shapes","cajetin"] {
        if sheet.native_baseline.get(key)!=current.get(key) { result[key]=current[key].clone(); }
    }
    if let Some(border)=&sheet.source_border {
        if sheet.source_border_baseline.as_ref()!=Some(border) {
            if border.on {
                if !result["border"].is_object(){result["border"]=serde_json::json!({});}
                for (key,value) in serde_json::to_value(border).unwrap().as_object().unwrap(){result["border"][key]=value.clone();}
            } else if let Some(object)=result.as_object_mut(){object.remove("border");}
        }
    }
    result
}

fn read_sheet(raw: &serde_json::Value) -> ingetrazo_core::composition::Sheet {
    use ingetrazo_core::composition::*;
    fn num(v:&serde_json::Value,k:&str,d:f64)->f64 { v.get(k).and_then(|v|v.as_f64()).unwrap_or(d) }
    fn txt(v:&serde_json::Value,k:&str,d:&str)->String { v.get(k).and_then(|v|v.as_str()).unwrap_or(d).into() }
    fn flag(v:&serde_json::Value,k:&str,d:bool)->bool { v.get(k).and_then(|v|v.as_bool()).unwrap_or(d) }
    fn rect(v:&serde_json::Value,d:RectMm)->RectMm {RectMm::new(num(v,"x_mm",d.x),num(v,"y_mm",d.y),num(v,"w_mm",d.w),num(v,"h_mm",d.h))}
    let paper=raw.get("paper").and_then(|v|serde_json::from_value(v.clone()).ok()).unwrap_or(PaperSize::A4);
    let mut sheet=Sheet::new(txt(raw,"name","Lámina 1"),paper,
        if flag(raw,"landscape",true) {Orientation::Landscape} else {Orientation::Portrait});
    sheet.items.clear();sheet.margin_mm=num(raw,"margin_mm",10.);
    for key in ["frames","texts","nortes","scalebars","shapes","cajetin"] {
        let entries:Vec<_>=if key=="cajetin" {raw.get(key).filter(|v|v.is_object()).into_iter().collect()}
            else {raw.get(key).and_then(|v|v.as_array()).map(|v|v.iter().collect()).unwrap_or_default()};
        for v in entries {
            let id=v.get("uid").and_then(|v|v.as_str()).and_then(|s|uuid::Uuid::parse_str(s).ok()).unwrap_or_else(uuid::Uuid::new_v4);
            let item=match key {
                "frames" => {
                    let mut f=ViewFrame::default();f.id=id;f.rect=rect(v,RectMm::new(20.,20.,170.,170.));
                    f.scale_n=num(v,"scale_n",100.);f.view_key=txt(v,"view_key","__current__");
                    f.style=txt(v,"style","sombreado");f.show_title=flag(v,"show_title",false);
                    f.title_text=txt(v,"title_text","");f.border=flag(v,"border",false);
                    f.perspective=flag(v,"perspective",false);f.rot_deg=num(v,"rot_deg",0.);
                    f.cam_target=v.get("cam_target").and_then(|v|serde_json::from_value(v.clone()).ok());
                    f.cam_yaw=v.get("cam_yaw").and_then(|v|v.as_f64());f.cam_pitch=v.get("cam_pitch").and_then(|v|v.as_f64());
                    f.cam_distance=v.get("cam_distance").and_then(|v|v.as_f64());f.cam_fov=v.get("cam_fov").and_then(|v|v.as_f64());
                    f.paper_bg=flag(v,"paper_bg",false);
                    f.cut_fill=txt(v,"cut_fill","solid");f.cut_fill_color=txt(v,"cut_fill_color","#595e69");f.cut_hatch_mm=num(v,"cut_hatch_mm",1.5);
                    f.pen_cut_mm=num(v,"pen_cut_mm",0.5);f.pen_profile_mm=num(v,"pen_profile_mm",0.35);f.pen_edge_mm=num(v,"pen_edge_mm",0.18);f.profiles=flag(v,"profiles",true);f.hidden_lines=flag(v,"hidden_lines",false);
                    f.annotations=flag(v,"annotations",false);f.section_marks=flag(v,"section_marks",false);
                    SheetItem::ViewFrame(f)
                },
                "cajetin" => {
                    let mut f=TitleBlock::default();f.id=id;f.rect=rect(v,RectMm::new(0.,0.,180.,33.));
                    fn field(v:&serde_json::Value,label:&str,key:&str)->String {
                        v.get("campos").and_then(|v|v.as_array()).and_then(|rows|rows.iter().find(|r|
                            r.get(0).and_then(|v|v.as_str()).map(|s|s.trim().to_uppercase()==label).unwrap_or(false)))
                            .and_then(|r|r.get(1)).and_then(|v|v.as_str()).unwrap_or_else(||v.get(key).and_then(|v|v.as_str()).unwrap_or("")).into()
                    }
                    f.project_name=field(v,"PROYECTO","proyecto");f.author=field(v,"AUTOR","autor");
                    f.date=field(v,"FECHA","fecha");f.scale_text=field(v,"ESCALA","escala");f.sheet_number=field(v,"LÁMINA","lamina");
                    f.drawing_title=field(v,"TÍTULO","drawing_title");SheetItem::TitleBlock(f)
                },
                "shapes" => SheetItem::Shape(ShapeItem{id,shape:serde_json::from_value(v.clone()).unwrap_or_default()}),
                "texts" => SheetItem::Text(TextItem{id,pos:(num(v,"x_mm",20.),num(v,"y_mm",20.)),text:txt(v,"text",""),font_size_pt:num(v,"size_pt",14.),bold:flag(v,"bold",false)}),
                "nortes" => SheetItem::NorthArrow(NorthArrowItem{id,pos:(num(v,"x_mm",20.),num(v,"y_mm",20.)),size_mm:num(v,"size_mm",18.),rot_deg:num(v,"angle_deg",0.)}),
                _ => SheetItem::GraphicScale(GraphicScaleItem{id,pos:(num(v,"x_mm",20.),num(v,"y_mm",20.)),scale_n:num(v,"scale_n",100.),length_mm:scale_bar_width(num(v,"scale_n",100.),v.get("segments").and_then(|v|v.as_u64()).unwrap_or(4) as usize),divisions:v.get("segments").and_then(|v|v.as_u64()).unwrap_or(4) as usize})
            };
            sheet.native_items.insert(id,v.clone());
            sheet.native_item_baselines.insert(id,sheet_item_model(&item).2);
            sheet.items.push(item);
        }
    }
    sheet.source_border=Some(SourcePaperBorder::from_native(&raw["border"]));sheet.source_border_baseline=sheet.source_border.clone();
    sheet.native_document=raw.clone();sheet.native_baseline=sheet_model(&sheet);sheet
}

fn dimension_model(d:&Dimension)->serde_json::Value {
    serde_json::json!({"a":d.start,"b":d.end,"offset":d.offset,"axis":d.axis,"layer":d.layer,"text":d.text_override})
}

fn view_model(v: &ingetrazo_core::saved_views::SavedView) -> serde_json::Value {
    serde_json::json!({
        "name":v.name, "target":v.target, "distance":v.distance, "yaw":v.yaw,
        "pitch":v.pitch, "fov":v.fov_deg, "parallel":!v.perspective || v.parallel,
        "hidden_layers":v.hidden_layers,"two_point":v.two_point,"hidden_objects":v.hidden_objects,
        "style":v.style,"section":v.section,"georef":v.georef,"shadows":v.shadows,"hidden_shown":v.hidden_shown
    })
}

fn section_model(v: &ingetrazo_core::section::SectionPlane) -> serde_json::Value {
    serde_json::json!({
        "point":v.point, "normal":v.normal, "name":v.name, "symbol":v.symbol,
        "active":v.active, "uid":v.id.as_u128() as u64
    })
}

fn image_plane_to_json(scene:&Scene,p:&ingetrazo_core::image_plane::ImagePlane)->serde_json::Value{
    let mut model=p.to_json();
    if scene.embedded_textures.contains_key(&p.path){model["texture"]=serde_json::json!({"embed":p.path});}
    let mut raw=overlay(&p.native_document,&p.native_baseline,model.clone());
    for key in ["name","opacity","layer","locked"]{
        if p.native_baseline.get(key)!=model.get(key){if let Some(value)=model.get(key){raw[key]=value.clone();}else if let Some(object)=raw.as_object_mut(){object.remove(key);}}
    }
    // A changed asset must not retain a stale embed that overrides its path
    // when the original reader extracts archives. Unrelated texture metadata
    // and no-op source representation are retained.
    if p.native_baseline.get("texture").and_then(|t|t.get("path")).and_then(|v|v.as_str())!=Some(p.path.as_str()){
        for key in ["path","embed"]{if let Some(value)=model["texture"].get(key){raw["texture"][key]=value.clone();}else if let Some(object)=raw["texture"].as_object_mut(){object.remove(key);}}
    }
    raw
}

fn scene_model(scene: &Scene) -> serde_json::Value {
    let loose = mesh_to_json(&scene.mesh);
    let groups_json: Vec<_> = scene.groups.iter().map(group_model).collect();
    let mut layers_json = Vec::new();
    for l in &scene.layers {
        layers_json.push(serde_json::json!({
            "name": l.name,
            "visible": l.visible,
            "locked": l.locked,
            "color": l.color,
        }));
    }

    let mut doc = loose;
    doc["display_style"]=serde_json::to_value(&scene.display_style).unwrap();
    doc["shadows"]=serde_json::to_value(&scene.shadows).unwrap();
    doc["groups"] = serde_json::json!(groups_json);
    doc["layers"] = serde_json::json!(layers_json);
    let mut names: Vec<_> = scene.materials.keys().collect();
    names.sort_by_key(|name| {
        scene.native_document.get("materials").and_then(|v| v.as_array())
            .and_then(|a| a.iter().position(|v| v.get("name").and_then(|v| v.as_str()) == Some(name.as_str())))
            .map(|i| (i, (*name).clone())).unwrap_or((usize::MAX, (*name).clone()))
    });
    doc["materials"] = serde_json::json!(names.iter().map(|name| {
        let mat = &scene.materials[*name];
        overlay(&mat.native_document, &mat.native_baseline, material_model(mat))
    }).collect::<Vec<_>>());
    doc["camera"] = serde_json::json!({
        "target": [scene.camera.target.x, scene.camera.target.y, scene.camera.target.z],
        "distance": scene.camera.distance,
        "yaw": scene.camera.yaw,
        "pitch": scene.camera.pitch,
        "perspective": scene.camera.perspective,
        "fov_deg": scene.camera.fov_deg,
    });
    doc["dimensions"] = serde_json::json!(scene.dimensions.iter().map(|d|{let mut live=d.clone();(live.start,live.end)=scene.dimension_points(d);overlay(&d.native_document,&d.native_baseline,dimension_model(&live))}).collect::<Vec<_>>());
    doc["image_planes"]=serde_json::json!(scene.image_planes.iter().map(|p|image_plane_to_json(scene,p)).collect::<Vec<_>>());
    doc["guides"] = serde_json::to_value(&scene.guides).unwrap_or(serde_json::json!([]));
    doc["text_labels"] = serde_json::to_value(&scene.text_labels).unwrap_or(serde_json::json!([]));
    let unit = match scene.unit {
        LengthUnit::Meters => "m", LengthUnit::Centimeters => "cm",
        LengthUnit::Millimeters => "mm", LengthUnit::Inches => "in", LengthUnit::Feet => "ft"
    };
    doc["plugin_data"] = scene.plugin_data.clone();
    doc["units"] = serde_json::json!({"length":unit,"precision":2});

    doc["saved_views"] = serde_json::json!(scene.saved_views.iter().map(|v|
        overlay(&v.native_document,&v.native_baseline,view_model(v))).collect::<Vec<_>>());
    doc["section_planes"] = serde_json::json!(scene.section_planes.iter().map(|v|
        overlay(&v.native_document,&v.native_baseline,section_model(v))).collect::<Vec<_>>());
    doc["section_view"] = serde_json::json!({
        "planes_hidden": !scene.show_section_planes, "cuts_hidden": !scene.show_section_cuts,
        "fill_hidden": !scene.show_section_fill
    });
    let mut shown=doc.get("hidden_view").and_then(|v|v.as_object()).cloned().unwrap_or_default();
    for (key,enabled) in [("objects",scene.show_hidden_objects),("geometry",scene.show_hidden_geometry)] {
        if enabled {shown.insert(key.into(),serde_json::Value::Bool(true));}else{shown.remove(key);}
    }
    if shown.is_empty(){doc.as_object_mut().unwrap().remove("hidden_view");}else{doc["hidden_view"]=serde_json::Value::Object(shown);}
    doc["georef"] = serde_json::json!({"datum": scene.geo_datum.as_ref().map(|d|
        serde_json::json!({"lat":d.lat,"lon":d.lon,"alt":d.alt,"north":d.north}))});
    doc["geo_points"] = serde_json::json!(scene.geo_points.iter().map(|p|
        overlay(&p.native_document,&p.native_baseline,point_model(p))).collect::<Vec<_>>());
    doc["geo_paths"] = serde_json::json!(scene.geo_paths.iter().map(|p|
        overlay(&p.native_document,&p.native_baseline,path_model(p))).collect::<Vec<_>>());
    doc["terrain"] = serde_json::json!(scene.terrain);
    doc["tile_layer"] = serde_json::json!(scene.tile_layer);
    doc["compositions"] = serde_json::json!(scene.sheets.iter().map(sheet_to_json).collect::<Vec<_>>());
    doc["active_sheet_index"] = serde_json::json!(scene.active_sheet_index);
    doc
}

pub fn scene_to_igz_document(scene: &Scene) -> serde_json::Value {
    let current = scene_model(scene);
    let mut doc = overlay(&scene.native_document, &scene.native_baseline, current.clone());
    if scene.native_baseline.get("hidden_view")!=current.get("hidden_view") {
        let mut shown=doc.get("hidden_view").and_then(|v|v.as_object()).cloned().unwrap_or_default();
        for (key,enabled) in [("objects",scene.show_hidden_objects),("geometry",scene.show_hidden_geometry)] {
            if enabled {shown.insert(key.into(),serde_json::Value::Bool(true));}else{shown.remove(key);}
        }
        if shown.is_empty(){doc.as_object_mut().unwrap().remove("hidden_view");}else{doc["hidden_view"]=serde_json::Value::Object(shown);}
    }
    if scene.native_baseline.get("image_planes")!=current.get("image_planes"){doc["image_planes"]=current["image_planes"].clone();}
    if scene.native_baseline.get("groups") != current.get("groups") {
        let mut protos = doc.get("protos").and_then(|v| v.as_array()).cloned().unwrap_or_default();
        let mut shared=std::collections::HashMap::new();
        doc["groups"] = serde_json::json!(scene.groups.iter().map(|g| group_to_json(g, &mut protos, &mut shared, &scene.mesh_shares)).collect::<Vec<_>>());
        if !protos.is_empty() { doc["protos"] = serde_json::json!(protos); }
    } else {
        // A view can reference a generated identity without a geometry edit.
        // Publish only those identities in the retained raw tree: unrelated
        // UID-less groups and every other authored field remain untouched.
        let references = scene.saved_views.iter()
            .filter_map(|view| view.hidden_objects.as_ref())
            .flatten().map(String::as_str).collect::<std::collections::HashSet<_>>();
        fn publish_references(raw: &mut serde_json::Value, groups: &[Group], references: &std::collections::HashSet<&str>) {
            if let Some(items) = raw.as_array_mut() {
                for (item, group) in items.iter_mut().zip(groups) {
                    if item.get("uid").and_then(serde_json::Value::as_str).is_none_or(str::is_empty) {
                        let identity = group.native_uid();
                        if references.contains(identity.as_str()) { item["uid"] = serde_json::Value::String(identity); }
                    }
                    if let Some(children) = item.get_mut("children") {
                        publish_references(children, &group.children, references);
                    }
                }
            }
        }
        if !references.is_empty() {
            if let Some(groups) = doc.get_mut("groups") { publish_references(groups, &scene.groups, &references); }
        }
    }
    doc
}

fn validate_native_save(scene: &Scene) -> Result<(), IgzError> {
    let current = scene_model(scene);
    // These modeled types still have incompatible original schemas. Preserve
    // original records for no-op saves; refuse edits instead of writing a file
    // the Python reader silently misinterprets.
    {
        for key in ["terrain"] {
            let value = current.get(key).unwrap();
            let nonempty = !value.is_null() && value.as_array().map(|a| !a.is_empty()).unwrap_or(true);
            if (scene.native_document.is_object() && scene.native_baseline.get(key) != Some(value))
                || (!scene.native_document.is_object() && nonempty) {
                return Err(IgzError::InvalidDocument(format!(
                    "editing {key} in an imported document requires a native schema adapter")));
            }
        }
    }
    fn validate_components(groups:&[Group],defs:&std::collections::HashMap<uuid::Uuid,ingetrazo_core::components::ComponentDefinition>,shares:&std::collections::HashMap<uuid::Uuid,Mesh>)->Result<(),IgzError> {
        for group in groups {
            if let Some(id)=group.component_definition {
                let def=defs.get(&id).ok_or_else(||IgzError::InvalidDocument("component definition not found".into()))?;
                let expected=if let Some(token)=group.mesh_share {shares.get(&token).ok_or_else(||IgzError::InvalidDocument("component mesh share not found".into()))?} else {&def.mesh};
                if mesh_to_json(&group.mesh)!=mesh_to_json(expected) && mesh_to_json(&group.mesh)!=mesh_to_json(&def.mesh) {
                    return Err(IgzError::InvalidDocument("component instance geometry diverged; use a shared definition edit or Make Unique".into()));
                }
            }
            validate_components(&group.children,defs,shares)?;
        }
        Ok(())
    }
    validate_components(&scene.groups,&scene.component_definitions,&scene.mesh_shares)?;
    for sheet in &scene.sheets {
        for item in &sheet.items {
            if let ingetrazo_core::composition::SheetItem::GraphicScale(bar)=item {
                if bar.divisions==0 || bar.scale_n<=0. || !bar.scale_n.is_finite()
                    || (bar.length_mm-scale_bar_width(bar.scale_n,bar.divisions)).abs()>1e-8 {
                    return Err(IgzError::InvalidDocument(
                        "graphic scale length cannot be represented by native segment sizing".into()));
                }
            }
        }
    }
    Ok(())
}

pub fn save_igz_format1(scene: &Scene, path: &Path) -> Result<(), IgzError> {
    validate_native_save(scene)?;
    if !scene.embedded_textures.is_empty() {
        return Err(IgzError::InvalidDocument("plain JSON cannot preserve embedded archive assets; use ZIP format".into()));
    }
    let envelope = IgzEnvelope {
        igz_format: 1,
        app_version: "0.3.15".to_string(),
        scene: scene_to_igz_document(scene),
    };
    let json_bytes = serde_json::to_vec_pretty(&envelope)?;
    let mut file = File::create(path)?;
    file.write_all(&json_bytes)?;
    Ok(())
}

pub fn save_igz_format2(scene: &Scene, path: &Path) -> Result<(), IgzError> {
    validate_native_save(scene)?;
    let mut doc = scene_to_igz_document(scene);
    let assets = assets::pack(scene, &mut doc)?;
    write_archive(doc, assets, path)
}

fn write_archive(
    document: serde_json::Value,
    assets: std::collections::BTreeMap<String, std::borrow::Cow<'_, [u8]>>,
    path: &Path,
) -> Result<(), IgzError> {
    let envelope = IgzEnvelope {
        igz_format: 2,
        app_version: "0.3.15".to_string(),
        scene: document,
    };
    // Serialize before opening the destination; callers of this low-level API
    // also retain their existing file when preparation fails.
    let json_bytes = serde_json::to_vec_pretty(&envelope)?;
    let file = File::create(path)?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for (name, bytes) in assets {
        zip.start_file(name, options)?;
        zip.write_all(&bytes)?;
    }
    zip.start_file("document.json", options)?;
    zip.write_all(&json_bytes)?;
    zip.finish()?;
    Ok(())
}

pub fn save_igz(scene: &Scene, path: &Path) -> Result<(), IgzError> {
    // Write next to the destination so a successful rename replaces it
    // atomically. Failed JSON/asset/ZIP writes never truncate the original.
    let parent=path.parent().filter(|p|!p.as_os_str().is_empty()).unwrap_or_else(||Path::new("."));
    let temp=parent.join(format!(".ingetrazo-save-{}.tmp",uuid::Uuid::new_v4()));
    validate_native_save(scene)?;
    let mut document = scene_to_igz_document(scene);
    let assets = assets::pack(scene, &mut document)?;
    let result = if !assets.is_empty() {
        write_archive(document, assets, &temp)
    } else {
        let envelope = IgzEnvelope { igz_format: 1, app_version: "0.3.15".into(), scene: document };
        serde_json::to_vec_pretty(&envelope).map_err(IgzError::from).and_then(|bytes| {
            File::create(&temp)?.write_all(&bytes)?;
            Ok(())
        })
    };
    if let Err(err)=result {
        let _=std::fs::remove_file(&temp);
        return Err(err);
    }
    if let Err(err)=File::open(&temp).and_then(|f|f.sync_all()) {
        let _=std::fs::remove_file(&temp);return Err(err.into());
    }
    if let Err(err)=std::fs::rename(&temp,path) {
        let _=std::fs::remove_file(&temp);
        return Err(err.into());
    }
    #[cfg(unix)]
    File::open(parent)?.sync_all()?;
    Ok(())
}

pub fn load_igz(path: &Path) -> Result<Scene, IgzError> {
    let mut file = File::open(path)?;
    let mut header = [0u8; 4];
    let n = file.read(&mut header)?;

    let mut embedded_textures = std::collections::HashMap::new();
    let json_text = if n >= 4 && &header == ZIP_MAGIC {
        let mut archive = zip::ZipArchive::new(File::open(path)?)?;
        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            let name = file.name().to_string();
            if name != "document.json" && !file.is_dir() {
                let mut bytes = Vec::new();
                file.read_to_end(&mut bytes)?;
                embedded_textures.insert(name, bytes);
            }
        }
        let mut doc = archive.by_name("document.json")?;
        let mut s = String::new();
        doc.read_to_string(&mut s)?;
        s
    } else {
        let mut content = Vec::new();
        content.extend_from_slice(&header[..n]);
        file.read_to_end(&mut content)?;
        String::from_utf8_lossy(&content).to_string()
    };

    let envelope: IgzEnvelope = serde_json::from_str(&json_text)?;
    let s_val = &envelope.scene;

    // Check if serialized as Rust Scene struct directly
    if s_val.get("mesh").is_some() && s_val.get("selection").is_some() {
        let mut scene: Scene = serde_json::from_value(s_val.clone())?;
        scene.embedded_textures = embedded_textures;
        scene.mesh.rebuild_spatial_index();
        for g in &mut scene.groups {
            g.mesh.rebuild_spatial_index();
        }
        return Ok(scene);
    }

    // Standard IngeTrazo format
    let mut scene = Scene::new();
    if let Some(planes)=s_val.get("image_planes").and_then(|v|v.as_array()){for raw in planes{scene.image_planes.push(ingetrazo_core::image_plane::ImagePlane::from_json(raw).map_err(IgzError::InvalidDocument)?);}scene.image_objects.publish(&scene.image_planes);}
    scene.display_style=ingetrazo_core::presentation::DocumentStyle::try_from_json(&s_val["display_style"]).map_err(IgzError::InvalidDocument)?;
    scene.shadows=ingetrazo_core::presentation::ShadowSettings::try_from_json(&s_val["shadows"]).map_err(IgzError::InvalidDocument)?;
    scene.embedded_textures = embedded_textures;
    scene.tile_layer=s_val.get("tile_layer").and_then(ingetrazo_core::georef::TileCapture::from_source_json);
    scene.native_document = s_val.clone();
    scene.plugin_data=s_val.get("plugin_data").cloned().unwrap_or_else(||serde_json::json!({}));
    scene.mesh = json_to_mesh(s_val);

    // Load component prototypes if present
    let mut proto_meshes = Vec::new();
    if let Some(protos_arr) = s_val.get("protos").and_then(|p| p.as_array()) {
        for p_val in protos_arr {
            proto_meshes.push(json_to_mesh(p_val));
        }
    }

    let proto_ids:Vec<_>=proto_meshes.iter().map(|_|uuid::Uuid::new_v4()).collect();
    scene.mesh_shares = proto_ids.iter().copied().zip(proto_meshes.iter().cloned()).collect();
    fn read_group(v: &serde_json::Value, protos: &[Mesh], proto_ids:&[uuid::Uuid], depth:usize, unnamed: &mut usize) -> Result<Group, IgzError> {
        if depth>64 {return Err(IgzError::InvalidDocument("group nesting limit exceeded".into()));}
        let name = v.get("name").and_then(|v|v.as_str()).filter(|s|!s.is_empty()).map(str::to_owned).unwrap_or_else(|| {
            let name = format!("Group {}", *unnamed);
            *unnamed += 1;
            name
        });
        let mut g = Group::new(name);
        if let Some(id)=v.get("uid").and_then(|v|v.as_str()).filter(|s|!s.is_empty()) {
            // Original UIDs are commonly 16 hex characters, not UUID32. Keep
            // their preserved on-disk string and a deterministic runtime key.
            g.id=ingetrazo_core::groups::source_uid_key(id);
        }
        if v.get("component").and_then(|v|v.as_bool())==Some(true) {
            if let Some(idx)=v.get("proto").and_then(|v|v.as_u64()) {
                g.component_definition=proto_ids.get(idx as usize).copied();
            }
        }
        g.mesh_share = v.get("proto").and_then(|v|v.as_u64()).and_then(|idx|proto_ids.get(idx as usize)).copied();
        g.mesh = if let Some(idx) = v.get("proto").and_then(|v| v.as_u64()) {
            protos.get(idx as usize).cloned().ok_or_else(||
                IgzError::InvalidDocument(format!("missing component prototype {idx}")))?
        } else { json_to_mesh(v) };
        if let Some(values) = v.get("xform").or_else(|| v.get("transform")).and_then(|v| v.as_array()) {
            if values.len() != 16 { return Err(IgzError::InvalidDocument("transform must have 16 numbers".into())); }
            let mut cols = [0.; 16];
            for (i, val) in values.iter().enumerate() {
                cols[i] = val.as_f64().ok_or_else(|| IgzError::InvalidDocument("non-numeric transform".into()))?;
            }
            g.transform = DMat4::from_cols_array(&cols);
        }
        g.layer = v.get("layer").and_then(|v| v.as_str()).map(str::to_owned);
        g.ifc_tag = v.get("ifc").filter(|v| !v.is_null()).cloned();
        g.hidden = v.get("hidden").and_then(|v| v.as_bool()).unwrap_or(false);
        g.billboard = v.get("billboard").map(|v| v == true || v == "mesh").unwrap_or(false);
        if let Some(children) = v.get("children").and_then(|v| v.as_array()) {
            g.children = children.iter().map(|v| read_group(v, protos, proto_ids, depth+1, unnamed)).collect::<Result<_,_>>()?;
        }
        g.mesh.rebuild_spatial_index();
        g.native_document = v.clone();
        g.native_baseline = group_model(&g);
        Ok(g)
    }
    if let Some(groups_arr) = s_val.get("groups").and_then(|g| g.as_array()) {
        let mut unnamed = 1;
        scene.groups = groups_arr.iter().map(|v| read_group(v, &proto_meshes, &proto_ids, 0, &mut unnamed)).collect::<Result<_,_>>()?;
    }

    fn register_definitions(groups:&[Group],defs:&mut std::collections::HashMap<uuid::Uuid,ingetrazo_core::components::ComponentDefinition>) {
        for g in groups {
            if let Some(id)=g.component_definition {
                defs.entry(id).or_insert_with(||ingetrazo_core::components::ComponentDefinition{mesh_share:None,mesh:g.mesh.clone(),children:g.children.clone()});
            }
            register_definitions(&g.children,defs);
        }
    }
    register_definitions(&scene.groups,&mut scene.component_definitions);
    fn bind_members(groups:&mut [Group],defs:&std::collections::HashMap<uuid::Uuid,ingetrazo_core::components::ComponentDefinition>) {
        fn assign(children:&mut [Group],templates:&[Group]) {
            for (child,template) in children.iter_mut().zip(templates) {
                child.definition_member=Some(template.id);
                assign(&mut child.children,&template.children);
            }
        }
        for g in groups {
            if let Some(def)=g.component_definition.and_then(|id|defs.get(&id)) {assign(&mut g.children,&def.children);}
            bind_members(&mut g.children,defs);
        }
    }
    bind_members(&mut scene.groups,&scene.component_definitions);

    if let Some(layers_arr) = s_val.get("layers").and_then(|l| l.as_array()).filter(|a| !a.is_empty()) {
        scene.layers.clear();
        for l_val in layers_arr {
            let name = l_val.get("name").and_then(|n| n.as_str()).unwrap_or(ingetrazo_core::layers::DEFAULT_LAYER).to_string();
            let visible = l_val.get("visible").and_then(|v| v.as_bool()).unwrap_or(true);
            let locked = l_val.get("locked").and_then(|l| l.as_bool()).unwrap_or(false);
            scene.layers.push(Layer {
                history_id:uuid::Uuid::new_v4(),
                name,
                visible,
                locked,
                color: l_val.get("color").and_then(|v| serde_json::from_value(v.clone()).ok()),
            });
        }
        if !scene.layers.iter().any(|layer|layer.name==ingetrazo_core::layers::DEFAULT_LAYER){scene.layers.insert(0,Layer::default());}
    }

    if let Some(cam) = s_val.get("camera") {
        if let Some(tgt) = cam.get("target").and_then(|t| t.as_array()) {
            if tgt.len() >= 3 {
                scene.camera.target = DVec3::new(
                    tgt[0].as_f64().unwrap_or(0.0),
                    tgt[1].as_f64().unwrap_or(0.0),
                    tgt[2].as_f64().unwrap_or(0.0),
                );
            }
        }
        if let Some(dist) = cam.get("distance").and_then(|d| d.as_f64()) {
            scene.camera.distance = dist;
        }
        if let Some(yaw) = cam.get("yaw").and_then(|y| y.as_f64()) {
            scene.camera.yaw = yaw;
        }
        if let Some(pitch) = cam.get("pitch").and_then(|p| p.as_f64()) {
            scene.camera.pitch = pitch;
        }
        if let Some(persp) = cam.get("perspective").and_then(|p| p.as_bool()) {
            scene.camera.perspective = persp;
        }
    }

    if let Some(materials_val) = s_val.get("materials") {
        if let Some(materials_obj) = materials_val.as_object() {
            for (name, mat_val) in materials_obj {
                if let Ok(mat) = serde_json::from_value::<Material>(mat_val.clone()) {
                    scene.materials.insert(name.clone(), mat);
                }
            }
        } else if let Some(materials_arr) = materials_val.as_array() {
            for m_item in materials_arr {
                if let Some(name) = m_item.get("name").and_then(|n| n.as_str()) {
                    let mut col = match name.to_lowercase().as_str() {
                        "metal_dark" => [0.22, 0.24, 0.26, 1.0],
                        "beech" | "wood" => [0.78, 0.58, 0.40, 1.0],
                        "concrete_smooth" | "concrete" => [0.72, 0.72, 0.70, 1.0],
                        _ => [0.80, 0.75, 0.70, 1.0],
                    };
                    if let Some(c_arr) = m_item.get("color").and_then(|c| c.as_array()) {
                        if c_arr.len() >= 3 {
                            col[0] = c_arr[0].as_f64().unwrap_or(col[0] as f64) as f32;
                            col[1] = c_arr[1].as_f64().unwrap_or(col[1] as f64) as f32;
                            col[2] = c_arr[2].as_f64().unwrap_or(col[2] as f64) as f32;
                            if c_arr.len() >= 4 {
                                col[3] = c_arr[3].as_f64().unwrap_or(col[3] as f64) as f32;
                            }
                        }
                    }
                    let mut mat = Material::new(name, col);
                    if let Some(op) = m_item.get("opacity").and_then(|o| o.as_f64()) {
                        mat.opacity = op as f32;
                        mat.color[3] = op as f32;
                    }
                    if let Some(tex) = m_item.get("texture") {
                        if let Some(emb) = tex.get("embed").and_then(|e| e.as_str()) {
                            mat.texture_embed = Some(emb.to_string());
                        }
                        if let Some(p) = tex.get("path").and_then(|p| p.as_str()) {
                            mat.texture_path = Some(p.to_string());
                        }
                    }
                    mat.native_document = m_item.clone();
                    mat.native_baseline = material_model(&mat);
                    scene.materials.insert(name.to_string(), mat);
                }
            }
        }
    }

    if let Some(sv_val) = s_val.get("saved_views").and_then(|v| v.as_array()) {
        for sv_item in sv_val {
            if let Ok(mut sv) = serde_json::from_value::<ingetrazo_core::saved_views::SavedView>(sv_item.clone()) {
                sv.fov_deg = sv_item.get("fov").or_else(|| sv_item.get("fov_deg")).and_then(|v| v.as_f64()).unwrap_or(45.);
                sv.perspective = !sv.parallel;
                sv.native_document = sv_item.clone();
                sv.native_baseline = view_model(&sv);
                scene.saved_views.push(sv);
            }
        }
    }

    if let Some(unit_val) = s_val.get("unit") {
        if let Ok(unit) = serde_json::from_value::<LengthUnit>(unit_val.clone()) {
            scene.unit = unit;
        }
    }

    if let Some(items)=s_val.get("dimensions").and_then(|v|v.as_array()) {
        for raw in items {if let Ok(mut dim)=serde_json::from_value::<Dimension>(raw.clone()) {
            dim.native_document=raw.clone();dim.native_baseline=dimension_model(&dim);scene.dimensions.push(dim);
        }}
    }

    scene.bind_dimensions();

    if let Some(guides_val) = s_val.get("guides") {
        if let Ok(guides) = serde_json::from_value::<Vec<Guide>>(guides_val.clone()) {
            scene.guides = guides;
        }
    }

    if let Some(tl_val) = s_val.get("text_labels") {
        if let Ok(labels) = serde_json::from_value::<Vec<ingetrazo_core::textlabel::TextLabel>>(tl_val.clone()) {
            scene.text_labels = labels;
        } else if let Some(arr) = tl_val.as_array() {
            for item in arr {
                if let Ok(lab) = serde_json::from_value::<ingetrazo_core::textlabel::TextLabel>(item.clone()) {
                    scene.text_labels.push(lab);
                }
            }
        }
    }

    if let Some(cam) = s_val.get("camera") {
        if let Some(fov) = cam.get("fov_deg").and_then(|v| v.as_f64()) { scene.camera.fov_deg = fov; }
    }
    if let Some(view) = s_val.get("section_view") {
        scene.show_section_planes = !view.get("planes_hidden").and_then(|v| v.as_bool()).unwrap_or(false);
        scene.show_section_cuts = !view.get("cuts_hidden").and_then(|v| v.as_bool()).unwrap_or(false);
        scene.show_section_fill = !view.get("fill_hidden").and_then(|v| v.as_bool()).unwrap_or(false);
    }
    if let Some(shown)=s_val.get("hidden_view") {
        scene.show_hidden_objects=shown.get("objects").is_some_and(|v| match v {serde_json::Value::Null=>false,serde_json::Value::Bool(b)=>*b,serde_json::Value::Number(n)=>n.as_f64()!=Some(0.),serde_json::Value::String(s)=>!s.is_empty(),serde_json::Value::Array(a)=>!a.is_empty(),serde_json::Value::Object(o)=>!o.is_empty()});
        scene.show_hidden_geometry=shown.get("geometry").is_some_and(|v| match v {serde_json::Value::Null=>false,serde_json::Value::Bool(b)=>*b,serde_json::Value::Number(n)=>n.as_f64()!=Some(0.),serde_json::Value::String(s)=>!s.is_empty(),serde_json::Value::Array(a)=>!a.is_empty(),serde_json::Value::Object(o)=>!o.is_empty()});
    }
    macro_rules! load_field {
        ($field:ident, $key:literal) => {
            if let Some(v) = s_val.get($key) {
                if let Ok(value) = serde_json::from_value(v.clone()) { scene.$field = value; }
            }
        }
    }
    if let Some(planes) = s_val.get("section_planes").and_then(|v| v.as_array()) {
        for plane in planes {
            if let (Some(point), Some(normal)) = (plane.get("point"),plane.get("normal")) {
                if let (Ok(point),Ok(normal)) = (serde_json::from_value::<DVec3>(point.clone()),serde_json::from_value::<DVec3>(normal.clone())) {
                    let mut p = ingetrazo_core::section::SectionPlane::new(point,normal,
                        plane.get("name").and_then(|v| v.as_str()).unwrap_or(""),
                        plane.get("symbol").and_then(|v| v.as_str()).unwrap_or(""));
                    p.active = plane.get("active").and_then(|v| v.as_bool()).unwrap_or(false);
                    if let Some(uid) = plane.get("uid").and_then(|v| v.as_u64()) { p.id = uuid::Uuid::from_u128(uid as u128); }
                    p.native_document = plane.clone();
                    p.native_baseline = section_model(&p);
                    scene.section_planes.push(p);
                }
            }
        }
    }
    if let Some(points) = s_val.get("geo_points").and_then(|v| v.as_array()) {
        for raw in points {
            if let Some(position) = raw.get("position") {
                if let Ok(position) = serde_json::from_value(position.clone()) {
                    let mut point = ingetrazo_core::georef::GeoPoint::new(position,
                        raw.get("name").and_then(|v| v.as_str()).unwrap_or(""),
                        raw.get("desc").and_then(|v| v.as_str()).unwrap_or(""));
                    point.native_document = raw.clone();
                    point.native_baseline = point_model(&point);
                    scene.geo_points.push(point);
                }
            }
        }
    }
    if let Some(paths) = s_val.get("geo_paths").and_then(|v| v.as_array()) {
        for raw in paths {
            if let Some(points) = raw.get("points") {
                if let Ok(points) = serde_json::from_value(points.clone()) {
                    let mut path = ingetrazo_core::georef::GeoPath::new(points,
                        raw.get("closed").and_then(|v| v.as_bool()).unwrap_or(false),
                        raw.get("name").and_then(|v| v.as_str()).unwrap_or(""));
                    path.surface = match raw.get("surface").and_then(|v| v.as_str()) {
                        Some("flat") => Some(ingetrazo_core::georef::SurfaceFillMode::Flat),
                        Some("draped") => Some(ingetrazo_core::georef::SurfaceFillMode::Draped),
                        _ => None
                    };
                    path.native_document = raw.clone();
                    path.native_baseline = path_model(&path);
                    scene.geo_paths.push(path);
                }
            }
        }
    }
    load_field!(terrain, "terrain");
    if let Some(sheets)=s_val.get("compositions").and_then(|v|v.as_array()) {
        scene.sheets=sheets.iter().map(read_sheet).collect();
    }
    load_field!(active_sheet_index, "active_sheet_index");
    if let Some(v) = s_val.get("georef").and_then(|v| v.get("datum")) {
        if let (Some(lat),Some(lon)) = (v.get("lat").and_then(|v| v.as_f64()),v.get("lon").and_then(|v| v.as_f64())) {
            scene.geo_datum = Some(ingetrazo_core::georef::SceneDatum::new(lat,lon,
                v.get("alt").and_then(|v| v.as_f64()).unwrap_or(0.),
                v.get("north").and_then(|v| v.as_f64()).unwrap_or(0.)));
        }
    }
    if let Some(u) = s_val.get("units").and_then(|v| v.get("length")).and_then(|v| v.as_str()) {
        scene.unit = match u {
            "cm" => LengthUnit::Centimeters, "mm" => LengthUnit::Millimeters,
            "in" => LengthUnit::Inches, "ft" => LengthUnit::Feet, _ => LengthUnit::Meters
        };
    }
    scene.tile_layer=s_val.get("tile_layer").and_then(ingetrazo_core::georef::TileCapture::from_source_json);
    scene.native_document = s_val.clone();
    scene.plugin_data=s_val.get("plugin_data").cloned().unwrap_or_else(||serde_json::json!({}));
    if s_val.get("display_style").is_some(){scene.show_section_fill=scene.display_style.section_fill;}else{scene.display_style.section_fill=scene.show_section_fill;}
    scene.native_baseline = scene_model(&scene);
    scene.mesh.rebuild_spatial_index();
    for g in &mut scene.groups {
        g.mesh.rebuild_spatial_index();
    }

    Ok(scene)
}

#[cfg(test)] mod sheet_shape_roundtrip_tests {
 use super::*;
 #[test] fn shape_edit_preserves_native_unknowns_and_other_categories() {
  let raw=serde_json::json!({"name":"mixed","paper":"A4","landscape":false,"images":[{"opaque":71}],"shapes":[{"kind":"terreno","x_mm":20.,"y_mm":30.,"w_mm":40.,"h_mm":25.,"ground":"hatch","z":4.,"hidden":false,"vendor":{"uid":71}}]});
  let mut sheet=read_sheet(&raw);assert_eq!(sheet_to_json(&sheet),raw);
  let index=sheet.items.iter().position(|i|matches!(i,ingetrazo_core::composition::SheetItem::Shape(_))).unwrap();
  sheet.items[index].set_pos(70.,80.);
  if let ingetrazo_core::composition::SheetItem::Shape(f)=&mut sheet.items[index] {f.shape.hidden=true;}
  assert!(!sheet.visible_item_indices().contains(&index));
  let saved=sheet_to_json(&sheet);assert_eq!(saved["images"],raw["images"]);assert_eq!(saved["shapes"][0]["vendor"],raw["shapes"][0]["vendor"]);assert_eq!(saved["shapes"][0]["x_mm"],70.);assert_eq!(saved["shapes"][0]["hidden"],true);
  let restored=read_sheet(&saved);assert_eq!(sheet_to_json(&restored),saved);
 }
}

#[cfg(test)]
mod source_border_runtime_tests {
    use super::*;
    #[test]
    fn dormant_authored_border_is_runtime_only_and_foreign_noop_is_retained() {
        let raw=serde_json::json!({"name":"Same","paper":"A4","landscape":false,"margin_mm":10.,"border":{"on":false,"color":"#ffffff","vendor":71}});
        let imported=read_sheet(&raw);
        assert_eq!(imported.effective_source_border().color,"#1e242c");
        assert_eq!(sheet_to_json(&imported)["border"],raw["border"]);
        let mut edited=imported.clone();let mut border=edited.effective_source_border();
        border.on=true;border.color="#ffffff".into();edited.set_source_border(border.clone()).unwrap();
        assert_eq!(sheet_to_json(&edited)["border"]["vendor"],71);
        border.on=false;edited.set_source_border(border).unwrap();
        assert!(sheet_to_json(&edited).get("border").is_none());
        assert_eq!(edited.effective_source_border().color,"#ffffff");
        assert_eq!(read_sheet(&sheet_to_json(&edited)).effective_source_border().color,"#1e242c");
    }
}
