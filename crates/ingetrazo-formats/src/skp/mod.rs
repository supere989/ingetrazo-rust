// SPDX-License-Identifier: GPL-3.0-or-later
//! Rust-only SketchUp import. Parse a neutral payload before preparing/applying a scene.
//! VFF record interpretation ports the pinned MIT OpenSKP reader; see OPENSKP-LICENSE.
mod placements;
mod reader;
mod textures;
use glam::{DMat4, DVec3};
use ingetrazo_core::{groups::Group, layers::Layer, materials::Material, mesh::Mesh, scene::Scene};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    path::Path,
};
#[derive(Debug, thiserror::Error)]
pub enum SkpError {
    #[error("SKP I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Invalid SKP: {0}")]
    Invalid(String),
    #[error("Required SKP feature not implemented: {0}")]
    Unsupported(String),
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EntityFlags {
    pub hidden: bool,
    pub display: u8,
    pub layer: Option<u64>,
    pub material: Option<u64>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub vertices: [u64; 2],
    pub flags: EntityFlags,
    pub curve: Option<u64>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Face {
    #[serde(default)]
    pub uv_transform: Option<[f64; 9]>,
    #[serde(default)]
    pub uv_transform_back: Option<[f64; 9]>,
    #[serde(default)]
    pub normal: Option<[f64; 3]>,
    pub loops: Vec<Vec<(u64, bool)>>,
    pub flags: EntityFlags,
    pub back_material: Option<u64>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Instance {
    pub definition: u64,
    pub name: String,
    pub matrix: [f64; 12],
    pub flags: EntityFlags,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Definition {
    #[serde(default)]
    pub face_order: Vec<u64>,
    pub name: String,
    pub vertices: BTreeMap<u64, [f64; 3]>,
    pub edges: BTreeMap<u64, Edge>,
    pub faces: BTreeMap<u64, Face>,
    pub instances: Vec<Instance>,
    pub billboard: bool,
    pub unsupported: BTreeSet<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerRecord {
    pub name: String,
    pub visible: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaterialRecord {
    pub color: [f64; 4],
    #[serde(default)]
    pub texture: Option<TextureRecord>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextureRecord {
    pub archive_name: String,
    pub bytes: Vec<u8>,
    pub width: f64,
    pub height: f64,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Payload {
    pub name: String,
    pub version: String,
    pub root: Definition,
    pub definitions: BTreeMap<u64, Definition>,
    pub layers: BTreeMap<u64, LayerRecord>,
    pub material_names: BTreeMap<u64, String>,
    pub materials: BTreeMap<String, MaterialRecord>,
    pub back_color: Option<[f64; 3]>,
    pub unsupported: BTreeSet<String>,
}
/// No Python interpreter, converter, application mutation, or external file extraction.
pub fn parse_skp_bytes(bytes: &[u8], name: &str) -> Result<Payload, SkpError> {
    reader::parse(bytes, name)
}
pub fn parse_skp(path: &Path) -> Result<Payload, SkpError> {
    let metadata = std::fs::metadata(path)?;
    if metadata.len() > 512 * 1024 * 1024 {
        return Err(SkpError::Invalid(
            "File exceeds bounded read admission".into(),
        ));
    }
    use std::io::Read;
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(512 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    parse_skp_bytes(
        &bytes,
        path.file_stem()
            .and_then(|n| n.to_str())
            .unwrap_or("SketchUp"),
    )
}
fn point(p: [f64; 3], x: DMat4) -> DVec3 {
    let v = x.transform_point3(DVec3::from_array(p) * 0.0254);
    DVec3::new(v.x as f32 as f64, v.y as f32 as f64, v.z as f32 as f64)
}
fn matrix(m: &[f64; 12]) -> DMat4 {
    DMat4::from_cols_array(&[
        m[0] as f32 as f64,
        m[3] as f32 as f64,
        m[6] as f32 as f64,
        0.,
        m[1] as f32 as f64,
        m[4] as f32 as f64,
        m[7] as f32 as f64,
        0.,
        m[2] as f32 as f64,
        m[5] as f32 as f64,
        m[8] as f32 as f64,
        0.,
        (m[9] * 0.0254) as f32 as f64,
        (m[10] * 0.0254) as f32 as f64,
        (m[11] * 0.0254) as f32 as f64,
        1.,
    ])
}
fn ring(d: &Definition, loop_: &[(u64, bool)]) -> Result<Vec<[f64; 3]>, SkpError> {
    if loop_.len() < 3 {
        return Err(SkpError::Invalid(
            "Face loop shorter than three edges".into(),
        ));
    }
    let mut out = Vec::new();
    for (i, (id, sense)) in loop_.iter().enumerate() {
        let edge = d
            .edges
            .get(id)
            .ok_or_else(|| SkpError::Invalid("Dangling face edge".into()))?;
        let next = d
            .edges
            .get(&loop_[(i + 1) % loop_.len()].0)
            .ok_or_else(|| SkpError::Invalid("Dangling next coedge".into()))?;
        let [a, b] = edge.vertices;
        let ash = next.vertices.contains(&a);
        let bsh = next.vertices.contains(&b);
        let vid = if ash != bsh {
            if bsh {
                b
            } else {
                a
            }
        } else {
            if *sense {
                a
            } else {
                b
            }
        };
        out.push(
            *d.vertices
                .get(&vid)
                .ok_or_else(|| SkpError::Invalid("Dangling ring vertex".into()))?,
        );
    }
    Ok(out)
}
fn attrs(p: &Payload, id: Option<u64>) -> Result<HashMap<String, serde_json::Value>, SkpError> {
    let mut a = HashMap::new();
    if let Some(id) = id {
        let name = p
            .material_names
            .get(&id)
            .ok_or_else(|| SkpError::Invalid(format!("Material {id} has no name join")))?;
        let mat = p
            .materials
            .get(name)
            .ok_or_else(|| SkpError::Invalid(format!("Material {name} lacks recipe")))?;
        if let Some(tex) = &mat.texture {
            let (sw, sh) = textures::tile_size(tex.width, tex.height);
            a.insert(
                "texture".into(),
                serde_json::json!({"embed":tex.archive_name,"sw":sw,"sh":sh}),
            );
        } else {
            a.insert("color".into(), serde_json::json!(&mat.color[..3]));
        }
        a.insert("mat".into(), serde_json::json!(name));
        if mat.color[3] < 0.999 {
            a.insert("opacity".into(), serde_json::json!(mat.color[3]));
        }
    }
    Ok(a)
}
fn collect(
    p: &Payload,
    d: &Definition,
    x: DMat4,
    inh: Option<u64>,
    layer: Option<u64>,
    mesh: &mut Mesh,
    _stack: &mut HashSet<u64>,
    depth: usize,
    projected: &HashSet<u64>,
) -> Result<(), SkpError> {
    if d.vertices.values().any(|v| !point(*v, x).is_finite()) {
        return Err(SkpError::Invalid(
            "Placed coordinate exceeds finite storage".into(),
        ));
    }
    if depth > 128 {
        return Err(SkpError::Invalid("Instance tree exceeds 128".into()));
    }
    if !d.unsupported.is_empty() {
        return Err(SkpError::Unsupported(
            d.unsupported.iter().cloned().collect::<Vec<_>>().join(", "),
        ));
    }
    if d.billboard {
        return Err(SkpError::Unsupported("face-camera component".into()));
    }
    for (face_id, face) in &d.faces {
        let flip = face.flags.material.is_none() && face.back_material.is_some();
        let material = face.flags.material.or(face.back_material).or(inh);
        let mut a = attrs(p, material)?;
        if !flip {
            let back = face.back_material.or(inh);
            if let Some(back) = back {
                a.insert(
                    "back".into(),
                    if Some(back) == material {
                        serde_json::json!(true)
                    } else {
                        serde_json::to_value(attrs(p, Some(back))?).unwrap()
                    },
                );
            }
        }
        // Pinned OpenSKP Face.layer is a raw id; the frozen adapter preserves
        // that numeric value. Instance-inherited layers are resolved names.
        let lay = if let Some(id) = face.flags.layer {
            a.insert("layer".into(), serde_json::json!(id));
            None
        } else {
            layer
                .map(|id| {
                    p.layers
                        .get(&id)
                        .map(|l| l.name.clone())
                        .ok_or_else(|| SkpError::Invalid("Missing inherited layer join".into()))
                })
                .transpose()?
        };
        if let Some(l) = &lay {
            a.insert("layer".into(), serde_json::json!(l));
        }
        let mut loops = Vec::new();
        for lp in &face.loops {
            let mut vertices = ring(d, lp)?
                .into_iter()
                .map(|v| mesh.add_vertex(point(v, x)))
                .collect::<Vec<_>>();
            if flip {
                vertices.reverse();
            }
            if vertices.iter().copied().collect::<HashSet<_>>().len() < 3 {
                return Err(SkpError::Invalid("Degenerate welded face".into()));
            }
            loops.push(vertices);
        }
        textures::bake(
            &mut a,
            face,
            d,
            &loops[0],
            mesh,
            flip,
            projected.contains(face_id),
        )?;
        let holes = loops.split_off(1);
        let f = mesh
            .add_face(loops.remove(0), holes)
            .ok_or_else(|| SkpError::Invalid("Face could not be built".into()))?;
        mesh.faces[f].attrs = a;
        mesh.faces[f].layer = lay;
    }
    // The frozen adapter softens flagged existing boundary edges, including hidden.
    for e in d.edges.values() {
        if e.flags.display & 0x19 == 0 {
            continue;
        }
        let a = point(
            *d.vertices
                .get(&e.vertices[0])
                .ok_or_else(|| SkpError::Invalid("Dangling soft edge vertex".into()))?,
            x,
        );
        let b = point(
            *d.vertices
                .get(&e.vertices[1])
                .ok_or_else(|| SkpError::Invalid("Dangling soft edge vertex".into()))?,
            x,
        );
        let va = mesh.vertices.iter().position(|v| v.position == a);
        let vb = mesh.vertices.iter().position(|v| v.position == b);
        if let (Some(a), Some(b)) = (va, vb) {
            if let Some(e) = mesh.find_edge(a, b) {
                mesh.edges[e].soft = true;
            }
        }
    }
    Ok(())
}
/// Prepare privately and reject unsupported required data before any caller Scene mutation.
pub fn scene_from_payload(p: &Payload) -> Result<Scene, SkpError> {
    textures::admit_assets(p)?;
    for d in std::iter::once(&p.root).chain(p.definitions.values()) {
        if d.vertices
            .values()
            .any(|v| v.iter().any(|x| !x.is_finite()) || !point(*v, DMat4::IDENTITY).is_finite())
        {
            return Err(SkpError::Invalid(
                "Nonfinite or unrepresentable vertex".into(),
            ));
        }
        for face in d.faces.values() {
            if face
                .uv_transform
                .iter()
                .chain(face.uv_transform_back.iter())
                .flatten()
                .any(|v| !v.is_finite())
            {
                return Err(SkpError::Invalid("Nonfinite texture mapping".into()));
            }
        }
        for ins in &d.instances {
            let x = matrix(&ins.matrix);
            if !x.is_finite() || x.determinant().abs() < 1e-16 {
                return Err(SkpError::Invalid("Singular or nonfinite placement".into()));
            }
        }
    }
    if p.materials.values().any(|m| {
        m.color
            .iter()
            .any(|c| !c.is_finite() || !(0.0..=1.0).contains(c))
    }) {
        return Err(SkpError::Invalid(
            "Material RGBA outside finite domain".into(),
        ));
    }

    if !p.root.unsupported.is_empty() {
        return Err(SkpError::Unsupported(
            p.root
                .unsupported
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .join(", "),
        ));
    }
    if !p.unsupported.is_empty() {
        return Err(SkpError::Unsupported(
            p.unsupported.iter().cloned().collect::<Vec<_>>().join(", "),
        ));
    }
    let mut scene = Scene::new();
    // Frozen Python Scene default; a model default tag is not imported again.
    scene.layers = vec![Layer::new("Layer 0")];
    for l in p.layers.values() {
        if matches!(l.name.as_str(), "Layer0" | "Untagged") {
            continue;
        }
        let mut layer = Layer::new(&l.name);
        layer.visible = l.visible;
        scene.layers.push(layer);
    }
    for name in p.material_names.values() {
        let m = p
            .materials
            .get(name)
            .ok_or_else(|| SkpError::Invalid("Material join missing recipe".into()))?;
        let mut material = Material::new(name, m.color.map(|c| c as f32));
        if let Some(tex) = &m.texture {
            material.color = [1., 1., 1., m.color[3] as f32];
            material.texture_embed = Some(tex.archive_name.clone());
            scene
                .embedded_textures
                .entry(tex.archive_name.clone())
                .or_insert_with(|| tex.bytes.clone());
        }
        material.native_document = if let Some(tex) = &m.texture {
            let (sw, sh) = textures::tile_size(tex.width, tex.height);
            serde_json::json!({"name":name,"color":null,"texture":{"embed":tex.archive_name,"sw":sw,"sh":sh}})
        } else {
            serde_json::json!({"name":name,"color":&m.color[..3]})
        };
        if m.color[3] < 0.999 {
            material.native_document["opacity"] = serde_json::json!(m.color[3]);
        }
        material.native_baseline = serde_json::json!({"name":material.name,"color":material.color,"opacity":material.opacity,"roughness":material.roughness,"texture":{"path":material.texture_path,"embed":material.texture_embed}});
        scene.materials.insert(name.clone(), material);
    }
    if let Some(c) = p.back_color {
        scene.display_style.back_color = Some(c);
    }
    if !p.root.faces.is_empty() {
        let mut mesh = Mesh::new();
        collect(
            p,
            &p.root,
            DMat4::IDENTITY,
            None,
            None,
            &mut mesh,
            &mut HashSet::new(),
            0,
            &textures::projected_faces(p, &p.root)?,
        )?;
        scene.groups.push(Group::with_mesh(&p.name, mesh));
    }
    placements::append(p, &mut scene)?;
    if scene.groups.is_empty() && !p.root.edges.is_empty() {
        return Err(SkpError::Unsupported("loose wire-only document".into()));
    }
    if scene.groups.is_empty() && !p.definitions.is_empty() {
        return Err(SkpError::Unsupported(
            "empty/library-only document semantics".into(),
        ));
    }
    if scene.groups.is_empty() {
        return Err(SkpError::Unsupported(
            "verified empty document import semantics".into(),
        ));
    }
    Ok(scene)
}
pub fn load_skp(path: &Path) -> Result<Scene, SkpError> {
    scene_from_payload(&parse_skp(path)?)
}
/// Replacement import seam: parsing/building fails without changing destination/history.
pub fn replace_from_skp(destination: &mut Scene, path: &Path) -> Result<(), SkpError> {
    let candidate = load_skp(path)?;
    *destination = candidate;
    Ok(())
}
