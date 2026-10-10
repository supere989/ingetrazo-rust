//! Source `_census`, `_shareable`, `_collect` and `_build_proto` placement planner.
use super::*;
use std::collections::HashMap;
type Key = (u64, Option<u64>);
#[derive(Clone)]
struct Place {
    definition: u64,
    x: DMat4,
    paint: Option<u64>,
    layer: Option<u64>,
}
struct Census {
    uses: HashMap<u64, usize>,
    polys: HashMap<u64, usize>,
    tagged: HashMap<u64, bool>,
    visits: usize,
    bounds: HashMap<u64, Option<([f64; 3], [f64; 3])>>,
}
fn definition(p: &Payload, id: u64) -> Result<&Definition, SkpError> {
    p.definitions
        .get(&id)
        .ok_or_else(|| SkpError::Invalid("Missing reachable definition".into()))
}
fn census(
    p: &Payload,
    d: &Definition,
    c: &mut Census,
    stack: &mut HashSet<u64>,
    depth: usize,
    x: DMat4,
) -> Result<(), SkpError> {
    if depth > 32 {
        return Err(SkpError::Invalid(
            "Placement depth admission exceeded".into(),
        ));
    }
    for ins in &d.instances {
        c.visits += 1;
        if c.visits > 1_000_000 {
            return Err(SkpError::Invalid(
                "Reachable placement admission exceeded".into(),
            ));
        }
        if !stack.insert(ins.definition) {
            return Err(SkpError::Invalid("Cyclic definition placement".into()));
        }
        *c.uses.entry(ins.definition).or_default() += 1;
        let next = product(x, matrix(&ins.matrix));
        if !next.is_finite() || next.determinant().abs() < 1e-16 {
            return Err(SkpError::Invalid("Invalid composed placement".into()));
        }
        if !c.bounds.contains_key(&ins.definition) {
            let d = definition(p, ins.definition)?;
            let mut bounds = None;
            for v in d.vertices.values() {
                let (lo, hi) = bounds.get_or_insert((*v, *v));
                for k in 0..3 {
                    lo[k] = lo[k].min(v[k]);
                    hi[k] = hi[k].max(v[k]);
                }
            }
            c.bounds.insert(ins.definition, bounds);
        }
        if let Some((lo, hi)) = c.bounds[&ins.definition] {
            for mask in 0..8 {
                let v = std::array::from_fn(|k| if mask & (1 << k) == 0 { lo[k] } else { hi[k] });
                if !point(v, next).is_finite() {
                    return Err(SkpError::Invalid(
                        "Placed coordinate bounds exceed finite source storage".into(),
                    ));
                }
            }
        }
        census(p, definition(p, ins.definition)?, c, stack, depth + 1, next)?;
        stack.remove(&ins.definition);
    }
    Ok(())
}
fn subtree(
    p: &Payload,
    id: u64,
    c: &mut Census,
    stack: &mut HashSet<u64>,
) -> Result<(usize, bool), SkpError> {
    if let Some(n) = c.polys.get(&id) {
        return Ok((*n, c.tagged[&id]));
    }
    if !stack.insert(id) {
        return Err(SkpError::Invalid("Cyclic definition placement".into()));
    }
    let d = definition(p, id)?;
    if d.billboard || !d.unsupported.is_empty() {
        return Err(SkpError::Unsupported(if d.billboard {
            "face-camera definition".into()
        } else {
            d.unsupported.iter().cloned().collect::<Vec<_>>().join(", ")
        }));
    }
    let mut n = d.faces.len();
    let mut tagged = false;
    for ins in &d.instances {
        let (child, t) = subtree(p, ins.definition, c, stack)?;
        n = n
            .checked_add(child)
            .ok_or_else(|| SkpError::Invalid("Subtree count overflow".into()))?;
        tagged |= ins.flags.layer.is_some() || t;
    }
    stack.remove(&id);
    c.polys.insert(id, n);
    c.tagged.insert(id, tagged);
    Ok((n, tagged))
}
fn has_proto(p: &Payload, id: u64, ids: &HashSet<u64>) -> Result<bool, SkpError> {
    for ins in &definition(p, id)?.instances {
        if ids.contains(&ins.definition) || has_proto(p, ins.definition, ids)? {
            return Ok(true);
        }
    }
    Ok(false)
}
// QMatrix4x4 keeps float components even when composing nested placements.
fn product(a: DMat4, b: DMat4) -> DMat4 {
    let aa = a.to_cols_array();
    let bb = b.to_cols_array();
    let mut out = [0.; 16];
    for col in 0..4 {
        for row in 0..4 {
            let mut v = 0f32;
            for k in 0..4 {
                v += (aa[k * 4 + row] as f32) * (bb[col * 4 + k] as f32);
            }
            out[col * 4 + row] = v as f64;
        }
    }
    DMat4::from_cols_array(&out)
}
struct Planner<'a> {
    p: &'a Payload,
    ids: HashSet<u64>,
    proto_order: Vec<Key>,
    proto_places: HashMap<Key, Vec<Place>>,
    tags: Vec<Place>,
}
impl Planner<'_> {
    fn collect(
        &mut self,
        d: &Definition,
        x: DMat4,
        paint: Option<u64>,
        layer: Option<u64>,
        mesh: &mut Mesh,
        children: &mut Vec<Place>,
        extract_tags: bool,
        depth: usize,
    ) -> Result<(), SkpError> {
        if depth > 32 {
            return Err(SkpError::Invalid(
                "Placement depth admission exceeded".into(),
            ));
        }
        super::collect(self.p, d, x, paint, layer, mesh, &mut HashSet::new(), depth)?;
        for ins in &d.instances {
            let child = definition(self.p, ins.definition)?;
            let next = product(x, matrix(&ins.matrix));
            if !next.is_finite() || next.determinant().abs() < 1e-16 {
                return Err(SkpError::Invalid("Invalid composed placement".into()));
            }
            let place = Place {
                definition: ins.definition,
                x: next,
                paint: ins.flags.material.or(paint),
                layer: ins.flags.layer.or(layer),
            };
            if child.billboard || child.unsupported.contains("image definition") {
                return Err(SkpError::Unsupported(
                    "image/billboard subtree extraction".into(),
                ));
            }
            if self.ids.contains(&ins.definition) {
                children.push(place);
            } else if extract_tags && ins.flags.layer.is_some() {
                self.tags.push(place);
            } else {
                self.collect(
                    child,
                    next,
                    place.paint,
                    place.layer,
                    mesh,
                    children,
                    extract_tags,
                    depth + 1,
                )?;
            }
        }
        Ok(())
    }
    fn record(&mut self, places: Vec<Place>) {
        for place in places {
            let key = (place.definition, place.paint);
            if !self.proto_places.contains_key(&key) {
                self.proto_order.push(key);
            }
            self.proto_places.entry(key).or_default().push(place);
        }
    }
    fn flat_group(&mut self, place: &Place, scene: &mut Scene) -> Result<(), SkpError> {
        let d = definition(self.p, place.definition)?;
        let mut mesh = Mesh::new();
        let mut children = Vec::new();
        // Source top/tagged group layer is on the Group, not inherited by its own faces.
        self.collect(
            d,
            place.x,
            place.paint,
            None,
            &mut mesh,
            &mut children,
            true,
            0,
        )?;
        self.record(children);
        if !mesh.faces.is_empty() {
            let name = if d.name.is_empty() {
                &self.p.name
            } else {
                &d.name
            };
            let mut g = Group::with_mesh(name, mesh);
            g.layer = layer_name(self.p, place.layer)?;
            if place.x != DMat4::IDENTITY {
                g.native_document = serde_json::json!({"axes":place.x.to_cols_array()});
            }
            scene.groups.push(g);
        }
        Ok(())
    }
}
fn layer_name(p: &Payload, id: Option<u64>) -> Result<Option<String>, SkpError> {
    id.map(|id| {
        p.layers
            .get(&id)
            .map(|l| l.name.clone())
            .ok_or_else(|| SkpError::Invalid("Missing placement layer".into()))
    })
    .transpose()
}
struct Proto {
    mesh: Mesh,
    id: uuid::Uuid,
    children: Vec<Place>,
}
fn build_proto(
    pl: &mut Planner<'_>,
    key: Key,
    protos: &mut HashMap<Key, Proto>,
    stack: &mut HashSet<Key>,
    order: &mut Vec<Key>,
) -> Result<(), SkpError> {
    if protos.contains_key(&key) {
        return Ok(());
    }
    if !stack.insert(key) {
        return Err(SkpError::Invalid("Cyclic prototype".into()));
    }
    order.push(key);
    let mut mesh = Mesh::new();
    let mut children = Vec::new();
    pl.collect(
        definition(pl.p, key.0)?,
        DMat4::IDENTITY,
        key.1,
        None,
        &mut mesh,
        &mut children,
        false,
        0,
    )?;
    for child in &children {
        build_proto(pl, (child.definition, child.paint), protos, stack, order)?;
    }
    protos.insert(
        key,
        Proto {
            mesh,
            id: uuid::Uuid::new_v4(),
            children,
        },
    );
    stack.remove(&key);
    Ok(())
}
fn place_proto(
    p: &Payload,
    place: &Place,
    protos: &HashMap<Key, Proto>,
    scene: &mut Scene,
    depth: usize,
) -> Result<Group, SkpError> {
    if depth > 32 {
        return Err(SkpError::Invalid("Prototype depth exceeded".into()));
    }
    let key = (place.definition, place.paint);
    let proto = &protos[&key];
    let d = definition(p, key.0)?;
    let mut g = Group::with_mesh(
        if d.name.is_empty() { &p.name } else { &d.name },
        proto.mesh.clone(),
    );
    g.mesh_share = Some(proto.id);
    g.transform = place.x;
    g.layer = layer_name(p, place.layer)?;
    scene
        .register_mesh_share(proto.id, proto.mesh.clone())
        .map_err(SkpError::Invalid)?;
    for kid in &proto.children {
        g.children
            .push(place_proto(p, kid, protos, scene, depth + 1)?);
    }
    Ok(g)
}
pub(super) fn append(p: &Payload, scene: &mut Scene) -> Result<(), SkpError> {
    let mut c = Census {
        uses: HashMap::new(),
        polys: HashMap::new(),
        tagged: HashMap::new(),
        visits: 0,
        bounds: HashMap::new(),
    };
    census(p, &p.root, &mut c, &mut HashSet::new(), 0, DMat4::IDENTITY)?;
    let mut ids = HashSet::new();
    // Use one memo across every definition, rather than repeated subtree scans.
    for id in c.uses.keys().copied().collect::<Vec<_>>() {
        subtree(p, id, &mut c, &mut HashSet::new())?;
    }
    for (&id, &uses) in &c.uses {
        let n = c.polys[&id];
        if uses >= 2 && !c.tagged[&id] && n >= 60 && n.saturating_mul(uses - 1) >= 400 {
            ids.insert(id);
        }
    }
    for ins in &p.root.instances {
        if c.uses[&ins.definition] == 1
            && !c.tagged[&ins.definition]
            && !has_proto(p, ins.definition, &ids)?
        {
            ids.insert(ins.definition);
        }
    }
    let mut pl = Planner {
        p,
        ids,
        proto_order: Vec::new(),
        proto_places: HashMap::new(),
        tags: Vec::new(),
    };
    for ins in &p.root.instances {
        let place = Place {
            definition: ins.definition,
            x: matrix(&ins.matrix),
            paint: ins.flags.material,
            layer: ins.flags.layer,
        };
        if pl.ids.contains(&ins.definition) {
            pl.record(vec![place]);
        } else {
            pl.flat_group(&place, scene)?;
        }
    }
    let mut i = 0;
    while i < pl.tags.len() {
        let place = pl.tags[i].clone();
        i += 1;
        if i > 1_000_000 {
            return Err(SkpError::Invalid(
                "Tagged placement admission exceeded".into(),
            ));
        }
        pl.flat_group(&place, scene)?;
    }
    let mut protos = HashMap::new();
    let mut order = Vec::new();
    for key in pl.proto_order.clone() {
        build_proto(&mut pl, key, &mut protos, &mut HashSet::new(), &mut order)?;
    }
    fold_equal(&mut pl, &mut protos, &mut order)?;
    for key in &pl.proto_order {
        for place in &pl.proto_places[key] {
            let group = place_proto(p, place, &protos, scene, 0)?;
            scene.groups.push(group);
        }
    }
    Ok(())
}

// Source folds equal built prototype contents, including children, in at most eight rounds.
fn fold_equal(
    pl: &mut Planner<'_>,
    protos: &mut HashMap<Key, Proto>,
    order: &mut Vec<Key>,
) -> Result<(), SkpError> {
    for _ in 0..8 {
        let mut seen = HashMap::<String, Key>::new();
        let mut remap = HashMap::new();
        let mut bytes = 0usize;
        for key in order.iter() {
            let pr = &protos[key];
            let d = definition(pl.p, key.0)?;
            let vertices = pr
                .mesh
                .vertices
                .iter()
                .map(|v| v.position.to_array())
                .collect::<Vec<_>>();
            let faces = pr
                .mesh
                .faces
                .iter()
                .map(|f| serde_json::json!([f.vertices, f.holes, f.attrs]))
                .collect::<Vec<_>>();
            let edges = pr
                .mesh
                .edges
                .iter()
                .map(|e| serde_json::json!([e.v0, e.v1, e.soft]))
                .collect::<Vec<_>>();
            let children = pr
                .children
                .iter()
                .map(|c| serde_json::json!([c.definition, c.paint, c.x.to_cols_array(), c.layer]))
                .collect::<Vec<_>>();
            let signature = serde_json::to_string(&canonical(serde_json::json!([
                d.name, vertices, faces, edges, children
            ])))
            .map_err(|e| SkpError::Invalid(e.to_string()))?;
            bytes = bytes
                .checked_add(signature.len())
                .ok_or_else(|| SkpError::Invalid("Prototype signature overflow".into()))?;
            if bytes > 256 * 1024 * 1024 {
                return Err(SkpError::Invalid(
                    "Prototype equality admission exceeded".into(),
                ));
            }
            if let Some(first) = seen.get(&signature) {
                remap.insert(*key, *first);
            } else {
                seen.insert(signature, *key);
            }
        }
        if remap.is_empty() {
            break;
        }
        for key in order.iter() {
            if let Some(keep) = remap.get(key) {
                if let Some(places) = pl.proto_places.remove(key) {
                    pl.proto_places
                        .entry(*keep)
                        .or_default()
                        .extend(places.into_iter().map(|mut p| {
                            p.definition = keep.0;
                            p.paint = keep.1;
                            p
                        }));
                }
            }
        }
        for pr in protos.values_mut() {
            for child in &mut pr.children {
                if let Some(keep) = remap.get(&(child.definition, child.paint)) {
                    child.definition = keep.0;
                    child.paint = keep.1;
                }
            }
        }
        for key in remap.keys() {
            protos.remove(key);
        }
        order.retain(|key| !remap.contains_key(key));
        pl.proto_order = order
            .iter()
            .copied()
            .filter(|key| pl.proto_places.contains_key(key))
            .collect();
    }
    Ok(())
}

fn canonical(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => {
            let sorted: std::collections::BTreeMap<_, _> = map.into_iter().collect();
            serde_json::Value::Object(sorted.into_iter().map(|(k, v)| (k, canonical(v))).collect())
        }
        serde_json::Value::Array(a) => {
            serde_json::Value::Array(a.into_iter().map(canonical).collect())
        }
        v => v,
    }
}
