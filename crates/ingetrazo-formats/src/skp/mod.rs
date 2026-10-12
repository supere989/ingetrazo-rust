pub mod app_import;
// SPDX-License-Identifier: GPL-3.0-or-later
//! Rust-only SketchUp import. Parse a neutral payload before preparing/applying a scene.
//! VFF record interpretation ports the pinned MIT OpenSKP reader; see OPENSKP-LICENSE.
mod legacy;
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
    pub uv_projected: bool,
    #[serde(default)]
    pub uv_projected_back: bool,
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
    pub image: bool,
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
    #[serde(default)]
    pub legacy_pages: Vec<LegacyPage>,
    #[serde(default)]
    pub legacy_dimensions: Vec<LegacyDimension>,
    #[serde(default)]
    pub legacy_texts: Vec<LegacyText>,
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegacyPage {
    pub name: String,
    pub hidden_layer_ids: Vec<u32>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegacyDimension {
    pub a: [f64; 3],
    pub b: [f64; 3],
    pub offset: f64,
    #[serde(default)]
    pub normal:Option<[f64;3]>,
    pub text: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegacyText {
    pub point: [f64; 3],
    pub label: Option<[f64; 3]>,
    pub text: String,
    pub hidden: bool,
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
// Image quads use QMatrix4x4.map(QVector3D), unlike the ordinary NumPy batch.
fn image_point(p:[f64;3],x:DMat4)->DVec3{
 let p=p.map(|v|(v*0.0254)as f32);let m=x.to_cols_array();DVec3::from_array(std::array::from_fn(|r|(((m[r]as f32*p[0]+m[4+r]as f32*p[1])+m[8+r]as f32*p[2])+m[12+r]as f32)as f64))
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
// Frozen Qt mesh registry: float32 positions, ties-even integer cells and
// float32 neighbour-distance arithmetic. Isolated from the global CAD weld.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum WeldAxis {
    Exact(i64),
    Wide(u64),
}
impl WeldAxis {
    fn new(value: f64) -> Self {
        let rounded = (value * 10000.).round_ties_even();
        // Beyond 2^53, each representable integer key is separated by >=2.
        // Python's arbitrary integers preserve that key; its +/-1 neighbours
        // cannot be keys of any other finite float32 coordinate.
        if rounded.abs() <= 9007199254740992. {
            Self::Exact(rounded as i64)
        } else {
            Self::Wide(rounded.to_bits())
        }
    }
    fn neighbour(self, delta: i64) -> Option<Self> {
        match self {
            Self::Exact(v) => Some(Self::Exact(v + delta)),
            Self::Wide(_) => {
                if delta == 0 {
                    Some(self)
                } else {
                    None
                }
            }
        }
    }
}
#[derive(Default)]
struct ImportWeld {
    cells: HashMap<[WeldAxis; 3], usize>,
}
impl ImportWeld {
    fn key(p: DVec3) -> [WeldAxis; 3] {
        [p.x, p.y, p.z].map(WeldAxis::new)
    }
    fn from_mesh(mesh: &Mesh) -> Self {
        let mut result = Self::default();
        for (id, v) in mesh.vertices.iter().enumerate() {
            result.cells.insert(Self::key(v.position), id);
        }
        result
    }
    fn lookup(&self, mesh: &Mesh, p: DVec3) -> Option<usize> {
        let k = Self::key(p);
        if let Some(&id) = self.cells.get(&k) {
            return Some(id);
        }
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    if dx == 0 && dy == 0 && dz == 0 {
                        continue;
                    }
                    let (Some(x), Some(y), Some(z)) =
                        (k[0].neighbour(dx), k[1].neighbour(dy), k[2].neighbour(dz))
                    else {
                        continue;
                    };
                    if let Some(&id) = self.cells.get(&[x, y, z]) {
                        let q = mesh.vertices[id].position;
                        let a = (q.x as f32) - (p.x as f32);
                        let b = (q.y as f32) - (p.y as f32);
                        let c = (q.z as f32) - (p.z as f32);
                        if (((a * a + b * b) + c * c) as f64) < 1e-8 {
                            return Some(id);
                        }
                    }
                }
            }
        }
        None
    }
    fn vertex(&mut self, mesh: &mut Mesh, p: DVec3) -> usize {
        if let Some(id) = self.lookup(mesh, p) {
            return id;
        }
        let id = mesh.vertices.len();
        mesh.vertices.push(ingetrazo_core::mesh::MeshVertex::new(p));
        self.cells.insert(Self::key(p), id);
        id
    }
}
// The frozen adapter's baked groups have no placement matrix. Preserve that
// distinction at the native codec boundary: an identity placement would make
// the original renderer choose its instanced pass instead of consolidated.
pub(super) fn preserve_baked_group(g: &mut Group) {
    g.native_document["xform"] = serde_json::Value::Null;
    g.native_baseline["xform"] = serde_json::json!(g.transform.to_cols_array());
    g.native_document["component"] = serde_json::json!(true);
    g.native_baseline["component"] = serde_json::json!(false);
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
    if d.billboard && !d.image {
        return Err(SkpError::Unsupported("face-camera component".into()));
    }
    let image_bounds=if d.image {textures::image_bounds(d)?}else{None};
    let mut weld = ImportWeld::from_mesh(mesh);
    for (face_id, face) in &d.faces {
        let flip = !d.image && face.flags.material.is_none() && face.back_material.is_some();
        let material = if d.image {face.flags.material.or(inh)} else {face.flags.material.or(face.back_material).or(inh)};
        let mut a = attrs(p, material)?;
        if !flip && !d.image {
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
        let lay = if d.image {None} else if let Some(id) = face.flags.layer {
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
        for lp in face.loops.iter().take(if d.image {1} else {usize::MAX}) {
            let mut points = ring(d, lp)?;
            if flip {
                points.reverse();
            }
            let vertices = points
                .into_iter()
                .map(|v| weld.vertex(mesh, if d.image {image_point(v,x)}else{point(v, x)}))
                .collect::<Vec<_>>();
            if vertices.iter().copied().collect::<HashSet<_>>().len() < 3 {
                return Err(SkpError::Invalid("Degenerate welded face".into()));
            }
            loops.push(vertices);
        }
        if d.image {textures::bake_image(&mut a,d,face,&loops[0],mesh,image_bounds)?;} else {textures::bake(
            &mut a,
            face,
            d,
            &loops[0],
            mesh,
            flip,
            projected.contains(face_id),
        )?;}
        let holes = loops.split_off(1);
        let f = mesh
            .add_face(loops.remove(0), holes)
            .ok_or_else(|| SkpError::Invalid("Face could not be built".into()))?;
        mesh.faces[f].attrs = a;
        mesh.faces[f].layer = lay;
    }
    // The frozen adapter softens flagged existing boundary edges, including hidden.
    for e in d.edges.values().filter(|_|!d.image) {
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
        let va = weld.lookup(mesh, a);
        let vb = weld.lookup(mesh, b);
        if let (Some(a), Some(b)) = (va, vb) {
            if let Some(e) = mesh.find_edge(a, b) {
                mesh.edges[e].soft = true;
            }
        }
    }
    mesh.rebuild_spatial_index();
    Ok(())
}
/// Prepare privately and reject unsupported required data before any caller Scene mutation.
fn annotation_point(p: [f64; 3]) -> Result<DVec3, SkpError> {
    let point = DVec3::from_array(p).map(|v| ((v * 0.0254) as f32) as f64);
    if !point.is_finite() {
        return Err(SkpError::Invalid(
            "Annotation exceeds finite float32 storage".into(),
        ));
    }
    Ok(point)
}
pub fn scene_from_payload(p: &Payload) -> Result<Scene, SkpError> {
    scene_from_payload_policy(p, false)
}
fn scene_from_payload_policy(p: &Payload, verified_empty: bool) -> Result<Scene, SkpError> {
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
            if face.loops.is_empty() || face.loops.iter().any(Vec::is_empty) {return Err(SkpError::Invalid("Face requires nonempty loops".into()));}
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
    // SKP imports start a new source document. An absent IGZ dimension_style
    // intentionally means legacy ticks, so preserve the complete new-document
    // default explicitly rather than changing the general IGZ reader.
    scene.native_document["dimension_style"] = serde_json::json!({
        "decimals": 2, "units": "m", "font_size": 9, "color": [45,55,75],
        "norma": "iso", "base_step_mm": 8.0, "ends": "arrow"
    });
    scene.native_baseline["terrain"] = serde_json::json!(scene.terrain);
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
        scene.native_document["back_face_color"] = serde_json::json!(c);
        // This is authored metadata on a newly imported native scene, not an
        // edit to a preserved original terrain schema.
        scene.native_baseline["terrain"] = serde_json::json!(scene.terrain);
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
        let mut group=Group::with_mesh(&p.name, mesh);
        preserve_baked_group(&mut group);
        scene.groups.push(group);
    }
    placements::append(p, &mut scene)?;
    for d in &p.legacy_dimensions {
        if !d.offset.is_finite() || !((d.offset * 0.0254) as f32).is_finite() {
            return Err(SkpError::Invalid(
                "Annotation offset float32 overflow".into(),
            ));
        }
        let a = annotation_point(d.a)?;
        let b = annotation_point(d.b)?;
        let delta = (b - a).map(|v| (v as f32) as f64);
        if !delta.is_finite() {
            return Err(SkpError::Invalid(
                "Annotation segment float32 overflow".into(),
            ));
        }
        if delta.length() < 1e-9 {
            continue;
        }
        let normal=d.normal.map(|p|DVec3::from_array(p).map(|v|(v as f32) as f64)).unwrap_or(DVec3::Z);
        if !normal.is_finite(){return Err(SkpError::Invalid("Annotation normal float32 overflow".into()));}
        let cross=|a:DVec3,b:DVec3|DVec3::new(((a.y as f32)*(b.z as f32)-(a.z as f32)*(b.y as f32)) as f64,((a.z as f32)*(b.x as f32)-(a.x as f32)*(b.z as f32)) as f64,((a.x as f32)*(b.y as f32)-(a.y as f32)*(b.x as f32)) as f64);
        let mut perpendicular=cross(normal,delta);
        if perpendicular.length()<1e-9 {perpendicular=cross(DVec3::Z,delta);}
        if perpendicular.length() < 1e-9 {
            perpendicular = cross(DVec3::X,delta);
        }
        // QVector3D normalizes using its float32 hypot, then float32 division.
        let length = (perpendicular.x as f32)
            .hypot(perpendicular.y as f32)
            .hypot(perpendicular.z as f32);
        if !length.is_finite() || length <= 0. {
            return Err(SkpError::Invalid(
                "Annotation normalization exceeds finite float32 arithmetic".into(),
            ));
        }
        let offset =
            perpendicular.map(|v| (((v as f32) / length) * ((d.offset * 0.0254) as f32)) as f64);
        if !offset.is_finite() {
            return Err(SkpError::Invalid(
                "Annotation derived offset float32 overflow".into(),
            ));
        }
        scene
            .dimensions
            .push(ingetrazo_core::dimensions::Dimension::new(a, b, offset));
    }
    for t in &p.legacy_texts {
        if t.hidden || t.text.trim().is_empty() {
            continue;
        }
        let a = annotation_point(t.point)?;
        let label = t.label.map(annotation_point).transpose()?.unwrap_or(a);
        if !(label - a).map(|v| (v as f32) as f64).is_finite() {
            return Err(SkpError::Invalid(
                "Annotation text offset float32 overflow".into(),
            ));
        }
        scene
            .text_labels
            .push(ingetrazo_core::textlabel::TextLabel::new(
                a,
                (label - a).map(|v| (v as f32) as f64),
                t.text.trim(),
            ));
    }
    if scene.groups.is_empty() && !p.root.edges.is_empty() {
        return Err(SkpError::Unsupported("loose wire-only document".into()));
    }
    if scene.groups.is_empty() && !p.definitions.is_empty() {
        return Err(SkpError::Unsupported(
            "empty/library-only document semantics".into(),
        ));
    }
    if scene.groups.is_empty() && !verified_empty {
        return Err(SkpError::Unsupported(
            "verified empty document import semantics".into(),
        ));
    }
    scene.bind_dimensions();
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

#[cfg(test)]
mod import_weld_tests {
    use super::*;
    #[test]
    fn public_annotations_refuse_float32_overflow_before_publication() {
        let base:Payload=serde_json::from_str(include_str!("../../../../docs/audits/skp-native-legacy/component-proof/vff35/hole-paint-layer-payload.json")).unwrap();
        for (a, b, offset) in [
            ([1e300, 0., 0.], [0., 0., 0.], 1.),
            ([-1e40, 0., 0.], [1e40, 0., 0.], 1.),
            ([0., 0., 0.], [1., 0., 0.], 1e300),
            ([0., 0., 0.], [1., 0., 0.], f64::NAN),
        ] {
            let mut p = base.clone();
            p.legacy_dimensions.push(LegacyDimension {
                a,
                b,
                offset,
                text: String::new(),
                normal:None,
            });
            assert!(matches!(scene_from_payload(&p), Err(SkpError::Invalid(_))));
        }
        let mut p = base.clone();
        p.legacy_texts.push(LegacyText {
            point: [0., 0., 0.],
            label: Some([1e300, 0., 0.]),
            text: "label".into(),
            hidden: false,
        });
        assert!(matches!(scene_from_payload(&p), Err(SkpError::Invalid(_))));
        assert!(scene_from_payload(&base).is_ok());
    }
    #[test]
    fn public_image_payload_empty_loops_refuse_before_scene_build() {
        let base:Payload=serde_json::from_str(include_str!("../../../../docs/audits/skp-native-legacy/component-proof/vff35/hole-paint-layer-payload.json")).unwrap();
        for loops in [Vec::new(),vec![Vec::new()]] {
            let mut p=base.clone();p.root.image=true;p.root.faces.values_mut().next().unwrap().loops=loops;
            assert!(matches!(scene_from_payload(&p),Err(SkpError::Invalid(_))));
        }
    }
    #[test]
    fn adopted_back_color_preserves_geometry_style_and_native_save() {
        let base:Payload=serde_json::from_str(include_str!("../../../../docs/audits/skp-native-legacy/component-proof/vff35/hole-paint-layer-payload.json")).unwrap();
        let plain=scene_from_payload(&base).unwrap();let mut painted=base.clone();painted.back_color=Some([0.2,0.3,0.4]);
        let scene=scene_from_payload(&painted).unwrap();
        assert_eq!(scene.display_style,plain.display_style);
        let mut a=crate::igz::scene_to_igz_document(&plain);let mut b=crate::igz::scene_to_igz_document(&scene);
        fn clear_uids(v:&mut serde_json::Value){match v {serde_json::Value::Object(o)=>{o.remove("uid");for v in o.values_mut(){clear_uids(v);}},serde_json::Value::Array(a)=>for v in a {clear_uids(v);},_=>{}}}
        clear_uids(&mut a);clear_uids(&mut b);
        for key in ["groups","protos","dimensions","text_labels","materials","layers","display_style"] {assert_eq!(a.get(key),b.get(key),"{key}");}
        assert_eq!(b["back_face_color"],serde_json::json!([0.2,0.3,0.4]));
        let expected=serde_json::json!({"decimals":2,"units":"m","font_size":9,"color":[45,55,75],"norma":"iso","base_step_mm":8.0,"ends":"arrow"});
        assert_eq!(a["dimension_style"],expected);
        assert_eq!(b["dimension_style"],expected);
        let path=std::env::temp_dir().join(format!("skp-back-color-{}.igz",uuid::Uuid::new_v4()));
        crate::igz::save_igz_format1(&scene,&path).unwrap();let saved:serde_json::Value=serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();std::fs::remove_file(&path).unwrap();
        assert_eq!(saved["scene"]["back_face_color"],serde_json::json!([0.2,0.3,0.4]));
        assert_eq!(saved["scene"]["dimension_style"],expected);
        let mut edited=scene.clone();edited.native_baseline["terrain"]=serde_json::json!({"unexpected":"terrain"});
        assert!(crate::igz::save_igz_format1(&edited,&path).is_err());assert!(!path.exists());
        let mut old=scene.clone();old.native_baseline=serde_json::Value::Null;assert!(crate::igz::save_igz_format1(&old,&path).is_err());assert!(!path.exists());
    }
    #[test]
    fn baked_and_shared_runtime_metadata_survive_codec_and_typed_edits() {
        let flat:Payload=serde_json::from_str(include_str!("../../../../docs/audits/skp-native-legacy/component-proof/vff35/hole-paint-layer-payload.json")).unwrap();
        let mut scene=scene_from_payload(&flat).unwrap();
        let raw=crate::igz::scene_to_igz_document(&scene);
        assert_eq!(raw["groups"][0]["xform"],serde_json::Value::Null);
        assert_eq!(raw["groups"][0]["component"],serde_json::json!(true));
        assert!(raw["groups"][0].get("proto").is_none());
        let id=scene.groups[0].id;
        scene.make_component(id).unwrap();
        let made=crate::igz::scene_to_igz_document(&scene);
        assert!(made["groups"][0]["proto"].is_number());
        assert!(made["groups"][0]["xform"].is_array());
        scene.make_component_unique(id).unwrap();
        let edited=crate::igz::scene_to_igz_document(&scene);
        assert_eq!(edited["groups"][0]["component"],serde_json::json!(true));
        assert!(edited["groups"][0].get("proto").is_none());
        assert!(edited["groups"][0]["xform"].is_null());
        let shared:Payload=serde_json::from_str(include_str!("../../../../docs/audits/skp-real-model-qt-runtime-replay/shared-eight-payload.json")).unwrap();
        let mut scene=scene_from_payload(&shared).unwrap();
        let raw=crate::igz::scene_to_igz_document(&scene);
        for g in raw["groups"].as_array().unwrap(){assert_eq!(g["component"],serde_json::json!(true));assert!(g["proto"].is_number());assert!(g["xform"].is_array());}
        let old=scene.groups[0].mesh.vertices[0].position;
        scene.groups[0].mesh.vertices[0].position.x+=1.;
        let changed=crate::igz::scene_to_igz_document(&scene);
        assert_ne!(changed["groups"][0]["proto"],changed["groups"][1]["proto"]);
        assert_eq!(scene.groups[1].mesh.vertices[0].position,old);
    }

    #[test]
    fn source_keys_neighbour_distance_and_wide_identity() {
        let mut mesh = Mesh::new();
        let mut w = ImportWeld::default();
        let a = w.vertex(&mut mesh, DVec3::new(0.000049, 0., 0.));
        assert_eq!(a, w.vertex(&mut mesh, DVec3::new(0.000051, 0., 0.)));
        assert_ne!(a, w.vertex(&mut mesh, DVec3::new(0.000151, 0., 0.)));
        assert_eq!(WeldAxis::new(0.00005), WeldAxis::Exact(0));
        assert_eq!(WeldAxis::new(0.00015), WeldAxis::Exact(1)); // actual binary product is below 1.5
        let big = DVec3::new(f32::MAX as f64, 0., 0.);
        let id = w.vertex(&mut mesh, big);
        assert_eq!(Some(id), w.lookup(&mesh, big));
        assert_ne!(id, w.vertex(&mut mesh, -big));
        assert_ne!(
            id,
            w.vertex(&mut mesh, DVec3::new((f32::MAX / 2.) as f64, 0., 0.))
        );
    }
}
