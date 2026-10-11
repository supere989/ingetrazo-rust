// SPDX-License-Identifier: GPL-3.0-or-later
//! Source formats.meshexport/glTF front-paint collection. Group hierarchy is
//! baked; backside paint is intentionally absent and materials are double sided.
use super::GltfError;
use glam::{DVec3, Mat4, Vec3};
use ingetrazo_core::{groups::Group, mesh::Mesh, scene::Scene};
use serde_json::{json, Value};
use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    path::Path,
};

struct Primitive {
    info: Value,
    positions: Vec<[f64; 3]>,
    normals: Vec<[f64; 3]>,
    uvs: Vec<[f64; 2]>,
    indices: Vec<u32>,
    vertices: HashMap<[u64; 8], u32>,
}
fn truth(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(v) => *v,
        Value::Number(v) => v.as_f64() != Some(0.),
        Value::String(v) => !v.is_empty(),
        Value::Array(v) => !v.is_empty(),
        Value::Object(v) => !v.is_empty(),
    }
}
fn visible(scene: &Scene, layer: Option<&str>) -> bool {
    let name = layer
        .filter(|s| !s.is_empty())
        .unwrap_or(ingetrazo_core::layers::DEFAULT_LAYER);
    scene
        .layers
        .iter()
        .find(|l| l.name == name)
        .is_none_or(|l| l.visible)
}
fn num(v: &Value, key: &str, default: f64) -> f64 {
    v.get(key).and_then(Value::as_f64).unwrap_or(default)
}
fn yup(v: DVec3) -> [f64; 3] {
    [v.x, v.z, -v.y]
}
fn qt_normal(points: &[DVec3]) -> DVec3 {
    let mut n = DVec3::ZERO;
    for i in 0..points.len() {
        let a = points[i];
        let b = points[(i + 1) % points.len()];
        n.x += (a.y - b.y) * (a.z + b.z);
        n.y += (a.z - b.z) * (a.x + b.x);
        n.z += (a.x - b.x) * (a.y + b.y);
    }
    let n = n.as_vec3();
    let scale = n.abs().max_element();
    let reduced = if scale == 0. { Vec3::ZERO } else { n / scale };
    let length = scale * reduced.length();
    if length < 1e-9 {
        DVec3::Z
    } else {
        (n * (1. / length)).as_dvec3()
    }
}
fn uv_axes(normal: DVec3, rotation: f64) -> (Vec3, Vec3) {
    let n = normal.normalize_or_zero();
    let horizontal = n.x.hypot(n.y);
    let (u, v) = if horizontal < 1e-3 {
        (if n.z > 0. { Vec3::X } else { -Vec3::X }, Vec3::Y)
    } else {
        let u = DVec3::new(-n.y / horizontal, n.x / horizontal, 0.);
        (u.as_vec3(), n.cross(u).as_vec3())
    };
    let (s, c) = rotation.to_radians().sin_cos();
    (
        u * (c as f32) + v * (s as f32),
        v * (c as f32) - u * (s as f32),
    )
}
fn rounded(v: f64, places: i32) -> f64 {
    // CPython rounds the exact binary input to a decimal digit. Multiplying
    // first loses which side of a half tie it occupied. This cold exporter
    // path matches its decimal conversion; no saturating integer keys.
    format!("{v:.precision$}", precision = places as usize)
        .parse()
        .unwrap_or(v)
}
fn vertex_key(position: [f64; 3], normal: [f64; 3], uv: [f64; 2]) -> [u64; 8] {
    let bit = |v: f64, digits| {
        let v = rounded(v, digits);
        if v == 0. {
            0
        } else {
            v.to_bits()
        }
    };
    [
        bit(position[0], 6),
        bit(position[1], 6),
        bit(position[2], 6),
        bit(normal[0], 4),
        bit(normal[1], 4),
        bit(normal[2], 4),
        bit(uv[0], 6),
        bit(uv[1], 6),
    ]
}
fn source_texture<'a>(scene: &'a Scene, texture: &Value) -> Option<(String, Cow<'a, [u8]>)> {
    let path = texture
        .get("path")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty());
    let embed = texture.get("embed").and_then(Value::as_str);
    let name = path.or(embed)?;
    let bytes = embed
        .and_then(|k| scene.embedded_textures.get(k))
        .map(|v| Cow::Borrowed(v.as_slice()))
        .or_else(|| {
            path.and_then(|k| scene.embedded_textures.get(k))
                .map(|v| Cow::Borrowed(v.as_slice()))
        })
        .or_else(|| path.and_then(|p| std::fs::read(p).ok()).map(Cow::Owned))?;
    Some((name.into(), bytes))
}
fn add_mesh(
    scene: &Scene,
    mesh: &Mesh,
    world: Mat4,
    loose: bool,
    bulk: bool,
    prims: &mut Vec<Primitive>,
    lookup: &mut HashMap<String, usize>,
) -> Result<(), GltfError> {
    let world64 = world.as_dmat4();
    let inverse = world64.inverse().as_mat4().as_dmat4();
    let map = |p: DVec3| {
        if bulk {
            world64
                .transform_point3(p.as_vec3().as_dvec3())
                .as_vec3()
                .as_dvec3()
        } else {
            world.transform_point3(p.as_vec3()).as_dvec3()
        }
    };
    for (id, face) in mesh.faces.iter().enumerate() {
        if loose
            && (face.attrs.get("hidden").is_some_and(truth)
                || !visible(
                    scene,
                    face.attrs
                        .get("layer")
                        .and_then(Value::as_str)
                        .or(face.layer.as_deref()),
                ))
        {
            continue;
        }
        let points: Vec<_> = face
            .vertices
            .iter()
            .map(|&v| map(mesh.vertices[v].position))
            .collect();
        let normal = qt_normal(&points);
        let opacity = face
            .attrs
            .get("opacity")
            .and_then(Value::as_f64)
            .filter(|v| *v < 0.999)
            .map(|v| rounded(v, 3));
        let texture = face
            .attrs
            .get("texture")
            .filter(|v| v.get("path").or_else(|| v.get("embed")).is_some_and(truth));
        let color = face
            .attrs
            .get("color")
            .filter(|v| truth(v))
            .cloned()
            .unwrap_or(json!([0.96, 0.95, 0.925]));
        let image_name = texture
            .and_then(|v| v.get("path").or_else(|| v.get("embed")))
            .and_then(Value::as_str)
            .map(|s| {
                Path::new(s)
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned()
            });
        let key = json!([
            if texture.is_some() { "tex" } else { "color" },
            image_name
                .as_ref()
                .map(|s| json!(s))
                .unwrap_or(color.clone()),
            opacity
        ])
        .to_string();
        let index = if let Some(&i) = lookup.get(&key) {
            i
        } else {
            let i = prims.len();
            lookup.insert(key.clone(), i);
            prims.push(Primitive {
                info: json!({"color":color,"texture":texture,"map":image_name,"opacity":opacity}),
                positions: Vec::new(),
                normals: Vec::new(),
                uvs: Vec::new(),
                indices: Vec::new(),
                vertices: HashMap::new(),
            });
            i
        };
        let prim = &mut prims[index];
        if prim.info.get("mat").is_none() {
            if let Some(name) = face.attrs.get("mat").filter(|v| truth(v)) {
                prim.info["mat"] = name.clone();
            }
        }
        let (u, v) = uv_axes(normal, texture.map_or(0., |t| num(t, "rot", 0.)));
        for (a, b, c) in mesh.triangulate_face(id) {
            // The source does not flip a reflected exported ring. Its normal
            // is recomputed from the transformed face, unlike viewport instancing.
            for local in [a, b, c] {
                let p = map(local);
                if !p.is_finite() || !normal.is_finite() {
                    return Err(GltfError::Invalid("Nonfinite exported geometry".into()));
                }
                let uv = texture.map(|t| {
                    if let Some(map) = t
                        .get("uvw")
                        .and_then(Value::as_array)
                        .filter(|a| a.len() == 8)
                    {
                        // Source transformed_attrs uses a float Qt inverse;
                        // evaluating that covector at the rounded world point
                        // reproduces its transformed texture anchoring.
                        let n = |j: usize| map[j].as_f64().unwrap_or(0.);
                        let eval = |base: usize| {
                            let g = DVec3::new(n(base), n(base + 1), n(base + 2));
                            let gp = inverse.transpose().transform_vector3(g);
                            gp.dot(p) + n(base + 3) - gp.dot(world64.w_axis.truncate())
                        };
                        [eval(0), eval(4)]
                    } else {
                        let size = |key| {
                            let n = num(t, key, 1.);
                            if n.abs() > 1e-9 {
                                n
                            } else {
                                1.
                            }
                        };
                        let q = p.as_vec3();
                        [q.dot(u) as f64 / size("sw"), q.dot(v) as f64 / size("sh")]
                    }
                });
                if uv.is_some_and(|uv| uv.iter().any(|x| !x.is_finite())) {
                    return Err(GltfError::Invalid("Nonfinite texture coordinates".into()));
                }
                let pp = yup(p);
                let nn = yup(normal);
                let vertex_key = vertex_key(pp, nn, uv.unwrap_or([0., 0.]));
                let vertex = if let Some(&i) = prim.vertices.get(&vertex_key) {
                    i
                } else {
                    let i = u32::try_from(prim.positions.len())
                        .map_err(|_| GltfError::Invalid("Too many glTF vertices".into()))?;
                    prim.vertices.insert(vertex_key, i);
                    prim.positions.push(pp);
                    prim.normals.push(nn);
                    if let Some(uv) = uv {
                        prim.uvs.push([uv[0], 1. - uv[1]]);
                    }
                    i
                };
                prim.indices.push(vertex);
            }
        }
    }
    Ok(())
}
fn collect(scene: &Scene) -> Result<Vec<Primitive>, GltfError> {
    fn group(
        scene: &Scene,
        g: &Group,
        parent: Mat4,
        bulk: bool,
        prims: &mut Vec<Primitive>,
        lookup: &mut HashMap<String, usize>,
    ) -> Result<(), GltfError> {
        let world = parent * Mat4::from_cols_array(&g.transform.to_cols_array().map(|v| v as f32));
        add_mesh(scene, &g.mesh, world, false, bulk, prims, lookup)?;
        // Source world_mesh merges descendants regardless child visibility.
        for child in &g.children {
            group(scene, child, world, bulk, prims, lookup)?;
        }
        Ok(())
    }
    let mut prims = Vec::new();
    let mut lookup = HashMap::new();
    add_mesh(
        scene,
        &scene.mesh,
        Mat4::IDENTITY,
        true,
        false,
        &mut prims,
        &mut lookup,
    )?;
    for g in &scene.groups {
        if !g.hidden && !g.billboard && visible(scene, g.layer.as_deref()) {
            group(
                scene,
                g,
                Mat4::IDENTITY,
                !g.children.is_empty() || g.mesh.edges.len() * 2 + g.mesh.faces.len() * 4 >= 1024,
                &mut prims,
                &mut lookup,
            )?;
        }
    }
    Ok(prims)
}
fn finish(scene: &Scene, info: &Value) -> &'static str {
    let name = info.get("mat").and_then(Value::as_str);
    if let Some(chosen) = name
        .and_then(|n| scene.materials.get(n))
        .and_then(|m| m.native_document.get("finish"))
        .and_then(Value::as_str)
    {
        match chosen {
            "matte" => return "matte",
            "satin" => return "satin",
            "gloss" => return "gloss",
            "metal" => return "metal",
            "glass" => return "glass",
            "water" => return "water",
            _ => {}
        }
    }
    // Ordered source keyword families, at word starts. Short words end there.
    let rules: [(&str, &[&str]); 5] = [
        (
            "water",
            &[
                "agua",
                "water",
                "pool",
                "piscina",
                "pileta",
                "espejo_de_agua",
                "ocean",
                "océano",
                "oceano",
                "água",
            ],
        ),
        (
            "glass",
            &[
                "vidri",
                "glass",
                "cristal",
                "crystal",
                "vitral",
                "window_pane",
                "windowpane",
                "mampara",
                "vidro",
            ],
        ),
        (
            "metal",
            &[
                "metal", "acero", "steel", "alumin", "hierro", "iron", "ferro", "chrom", "cromo",
                "inox", "bronce", "bronze", "cobre", "copper", "latón", "laton", "brass",
                "galvaniz", "zinc", "zinco", "titani",
            ],
        ),
        (
            "gloss",
            &[
                "mármol", "marmol", "marble", "mármore", "marmore", "granit", "porcel", "cerámic",
                "ceramic", "azulej", "tile", "lacad", "lacquer", "laca", "pulid", "polish",
                "brillant", "glossy", "plástic", "plastic", "acrílic", "acrilic", "esmalt",
                "enamel", "gleam",
            ],
        ),
        (
            "satin",
            &[
                "madera", "wood", "madeira", "parquet", "laminad", "laminate", "cuero", "leather",
                "couro", "satin", "satinad", "semi",
            ],
        ),
    ];
    for text in [name, info.get("map").and_then(Value::as_str)]
        .into_iter()
        .flatten()
    {
        let text = text.to_lowercase().replace(['-', ' '], "_");
        for (kind, prefixes) in rules {
            for (i, _) in text.char_indices() {
                if i > 0
                    && text[..i]
                        .chars()
                        .next_back()
                        .is_some_and(|c| c.is_ascii_lowercase() || "áéíóúñç".contains(c))
                {
                    continue;
                }
                let word = &text[i..];
                if prefixes.iter().any(|s| word.starts_with(s)) {
                    return kind;
                }
                let exact = if kind == "water" {
                    &["lago", "lake", "mar", "sea"][..]
                } else if kind == "metal" {
                    &["aço", "aco"][..]
                } else {
                    &[]
                };
                if exact.iter().any(|s| {
                    word.starts_with(s)
                        && word[s.len()..]
                            .chars()
                            .next()
                            .is_none_or(|c| !c.is_ascii_lowercase() && !"áéíóúñç".contains(c))
                }) {
                    return kind;
                }
            }
        }
    }
    if info
        .get("opacity")
        .and_then(Value::as_f64)
        .is_some_and(|v| v < 0.999)
    {
        "glass"
    } else {
        "matte"
    }
}
struct Buffer {
    data: Vec<u8>,
    views: Vec<Value>,
    accessors: Vec<Value>,
}
impl Buffer {
    fn align(&mut self) {
        while self.data.len() % 4 != 0 {
            self.data.push(0);
        }
    }
    fn view(&mut self, bytes: &[u8], target: Option<u32>) -> usize {
        self.align();
        let mut view = json!({"buffer":0,"byteOffset":self.data.len(),"byteLength":bytes.len()});
        if let Some(t) = target {
            view["target"] = json!(t);
        }
        self.data.extend_from_slice(bytes);
        self.views.push(view);
        self.views.len() - 1
    }
    fn accessor(
        &mut self,
        bytes: &[u8],
        component: u32,
        count: usize,
        kind: &str,
        target: u32,
        bounds: Option<([f64; 3], [f64; 3])>,
    ) -> usize {
        let view = self.view(bytes, Some(target));
        let mut a = json!({"bufferView":view,"componentType":component,"count":count,"type":kind});
        if let Some((min, max)) = bounds {
            a["min"] = json!(min);
            a["max"] = json!(max);
        }
        self.accessors.push(a);
        self.accessors.len() - 1
    }
}
fn floats<const N: usize>(rows: &[[f64; N]]) -> Vec<u8> {
    rows.iter()
        .flatten()
        .flat_map(|x| (*x as f32).to_le_bytes())
        .collect()
}
pub(super) fn build(scene: &Scene) -> Result<(Value, Vec<u8>), GltfError> {
    let primitives = collect(scene)?;
    let mut buffer = Buffer {
        data: Vec::new(),
        views: Vec::new(),
        accessors: Vec::new(),
    };
    let mut materials = Vec::new();
    let mut images = Vec::new();
    let mut textures = Vec::new();
    let mut image_cache = HashMap::new();
    let mut extensions = HashSet::new();
    let mut names = HashSet::new();
    for (i, p) in primitives.iter().enumerate() {
        let raw = p
            .info
            .get("mat")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim();
        let mut base = raw
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || "._-".contains(c) {
                    c
                } else {
                    '_'
                }
            })
            .collect::<String>()
            .trim_matches(['.', '_', '-'])
            .to_string();
        if base.is_empty() {
            base = format!("mat{i}");
        }
        if base.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            base = format!("m_{base}");
        }
        let mut name = base.clone();
        let mut suffix = 2;
        while !names.insert(name.clone()) {
            name = format!("{base}_{suffix}");
            suffix += 1;
        }
        let mut material = json!({"name":name,"doubleSided":true,"pbrMetallicRoughness":{"metallicFactor":0.,"roughnessFactor":1.}});
        if let Some(tex) = p.info.get("texture").filter(|v| !v.is_null()) {
            let source = tex
                .get("path")
                .or_else(|| tex.get("embed"))
                .and_then(Value::as_str)
                .unwrap_or("");
            let prepared = image_cache.get(source).copied().or_else(|| {
                let (source, bytes) = source_texture(scene, tex)?;
                let name = p.info["map"].as_str().unwrap_or("texture.png");
                let view = buffer.view(&bytes, None);
                let jpeg = Path::new(name)
                    .extension()
                    .and_then(|s| s.to_str())
                    .is_some_and(|s| {
                        s.eq_ignore_ascii_case("jpg") || s.eq_ignore_ascii_case("jpeg")
                    });
                images.push(json!({"bufferView":view,"mimeType":if jpeg{"image/jpeg"}else{"image/png"},"name":name}));
                let index = textures.len();
                textures.push(json!({"source":images.len()-1,"sampler":0}));
                let cutout = image::load_from_memory(&bytes).ok().is_some_and(|img| {
                    let image = img.to_rgba8();
                    (0..32).any(|y| {
                        (0..32).any(|x| {
                            image.get_pixel(
                                ((2 * x + 1) * image.width() / 64).min(image.width() - 1),
                                ((2 * y + 1) * image.height() / 64).min(image.height() - 1),
                            )[3] < 32
                        })
                    })
                });
                image_cache.insert(source, (index, cutout));
                Some((index, cutout))
            });
            if let Some((index, cutout)) = prepared {
                material["pbrMetallicRoughness"]["baseColorTexture"] = json!({"index":index});
                if cutout {
                    material["alphaMode"] = json!("MASK");
                    material["alphaCutoff"] = json!(0.5);
                }
            } else {
                material["pbrMetallicRoughness"]["baseColorFactor"] = json!([1, 1, 1, 1]);
            }
        } else {
            let color = p.info["color"]
                .as_array()
                .ok_or_else(|| GltfError::Invalid("Non-array face color".into()))?;
            if color.len() != 3 {
                return Err(GltfError::Invalid(
                    "Face color must contain three channels".into(),
                ));
            }
            material["pbrMetallicRoughness"]["baseColorFactor"] =
                json!([color[0], color[1], color[2], 1.]);
        }
        let finish = finish(scene, &p.info);
        let (rough, metal, trans, ior) = match finish {
            "satin" => (0.45, 0., 0., 1.5),
            "gloss" => (0.12, 0., 0., 1.5),
            "metal" => (0.3, 1., 0., 1.5),
            "glass" => (0.02, 0., 1., 1.45),
            "water" => (0.03, 0., 0., 1.33),
            _ => (0.9, 0., 0., 1.5),
        };
        material["pbrMetallicRoughness"]["roughnessFactor"] = json!(rough);
        material["pbrMetallicRoughness"]["metallicFactor"] = json!(metal);
        material["extras"] = json!({"ingetrazo_finish":finish});
        if trans != 0. {
            material["extensions"]["KHR_materials_transmission"] =
                json!({"transmissionFactor":trans});
            extensions.insert("KHR_materials_transmission");
        }
        if ior != 1.5 {
            material["extensions"]["KHR_materials_ior"] = json!({"ior":ior});
            extensions.insert("KHR_materials_ior");
        }
        if let Some(op) = p.info["opacity"].as_f64() {
            if material["pbrMetallicRoughness"]
                .get("baseColorFactor")
                .is_none()
            {
                material["pbrMetallicRoughness"]["baseColorFactor"] = json!([1., 1., 1., 1.]);
            }
            material["pbrMetallicRoughness"]["baseColorFactor"][3] = json!(op);
            material["alphaMode"] = json!("BLEND");
            material.as_object_mut().unwrap().remove("alphaCutoff");
        }
        materials.push(material);
    }
    let mut output = Vec::new();
    for (material, p) in primitives.iter().enumerate() {
        if p.positions.is_empty() {
            continue;
        }
        let mut min = [f64::INFINITY; 3];
        let mut max = [f64::NEG_INFINITY; 3];
        for row in &p.positions {
            for j in 0..3 {
                min[j] = min[j].min(row[j]);
                max[j] = max[j].max(row[j]);
            }
        }
        let position = buffer.accessor(
            &floats(&p.positions),
            5126,
            p.positions.len(),
            "VEC3",
            34962,
            Some((min, max)),
        );
        let normal = buffer.accessor(
            &floats(&p.normals),
            5126,
            p.normals.len(),
            "VEC3",
            34962,
            None,
        );
        let mut attrs = json!({"POSITION":position,"NORMAL":normal});
        if !p.uvs.is_empty() {
            attrs["TEXCOORD_0"] =
                json!(buffer.accessor(&floats(&p.uvs), 5126, p.uvs.len(), "VEC2", 34962, None));
        }
        let indices = p
            .indices
            .iter()
            .flat_map(|n| n.to_le_bytes())
            .collect::<Vec<_>>();
        let index = buffer.accessor(&indices, 5125, p.indices.len(), "SCALAR", 34963, None);
        output.push(json!({"attributes":attrs,"indices":index,"material":material,"mode":4}));
    }
    let mut extras = json!({"generator":"IngeTrazo"});
    if let Some(geo) = &scene.geo_datum {
        extras["ingetrazo_geolocation"] = json!({"lat":geo.lat,"lon":geo.lon,"alt":geo.alt});
    }
    let mut document = json!({"asset":{"version":"2.0","generator":"IngeTrazo","extras":extras},"scene":0,"scenes":[{"nodes":if output.is_empty(){vec![]}else{vec![0]}}]});
    if !output.is_empty() {
        document["meshes"] = json!([{"name":"IngeTrazo","primitives":output}]);
        document["nodes"] = json!([{"mesh":0,"name":"IngeTrazo"}]);
    }
    if !buffer.accessors.is_empty() {
        document["accessors"] = json!(buffer.accessors);
        document["bufferViews"] = json!(buffer.views);
        document["buffers"] = json!([{"byteLength":buffer.data.len()}]);
    }
    if !materials.is_empty() {
        document["materials"] = json!(materials);
    }
    if !images.is_empty() {
        document["images"] = json!(images);
        document["textures"] = json!(textures);
        document["samplers"] = json!([{"wrapS":10497,"wrapT":10497}]);
    }
    if !extensions.is_empty() {
        let mut ext = extensions.into_iter().collect::<Vec<_>>();
        ext.sort();
        document["extensionsUsed"] = json!(ext);
    }
    buffer.align();
    Ok((document, buffer.data))
}
