//! Identity-owned packed closure: public membership first, then held topology/registry.
//! Public Vertex membership and source position snapshots are distinct from GPU rows.
use super::{
    FaceRestore, GraphState, VertexRestore,
    editable_mesh::{Mesh, Vertex},
};
use crate::{
    journal::SourceKey,
    owner_budget::{Kind, Lease, OwnerBudget},
};
use rustpython_vm::{
    AsObject, PyObjectRef, PyRef, VirtualMachine,
    builtins::{PyInt, PyList, PyTuple},
};
use std::collections::HashMap;
pub(super) struct Candidate {
    pub mesh: ingetrazo_core::mesh::Mesh,
    pub keys: Vec<(SourceKey, usize)>,
    assignments: Vec<(PyRef<Vertex>, uuid::Uuid)>,
    pub charge: Lease,
    field_charge: Lease,
    opaque_fields: usize,
}
impl Candidate {
    /// Scene publication retains both packed storage and typed field leases until
    /// the fresh native receipt is installed. This does not mutate the VM owner.
    pub(super) fn into_mesh_receipt(
        mut self,
    ) -> Result<(ingetrazo_core::mesh::Mesh, Lease), String> {
        self.charge.merge(self.field_charge)?;
        Ok((self.mesh, self.charge))
    }
    pub fn fields_changed(&self, live: &PyRef<Mesh>) -> Result<bool, String> {
        let owner = live.owner.read();
        let native = owner.mesh();
        if native.vertices.len() != self.mesh.vertices.len() {
            return Ok(true);
        }
        if native.faces.len() != self.mesh.faces.len()
            || native
                .faces
                .iter()
                .zip(&self.mesh.faces)
                .any(|(a, b)| a.attrs != b.attrs || a.layer != b.layer)
        {
            return Ok(true);
        }
        let mut positions = HashMap::new();
        positions
            .try_reserve(native.vertices.len())
            .map_err(|_| "Packed comparison positions refused")?;
        for vertex in &native.vertices {
            if positions.insert(vertex.uid, vertex.position).is_some() {
                return Err("Repeated native packed UID".into());
            }
        }
        Ok(self
            .mesh
            .vertices
            .iter()
            .any(|vertex| positions.get(&vertex.uid) != Some(&vertex.position)))
    }
    pub fn apply_history(
        self,
        live: &PyRef<Mesh>,
        state: uuid::Uuid,
        redo: bool,
    ) -> Result<(), String> {
        let Self {
            mesh,
            keys,
            assignments,
            charge,
            mut field_charge,
            opaque_fields,
        } = self;
        field_charge.reclassify(Kind::History)?;
        let result = {
            let mut owner = live.owner.write();
            owner
                .prepare_identity_history_replacement_as(mesh, keys, state, redo)
                .map(|ticket| ticket.apply())
        };
        let old = result?;
        for (vertex, uid) in assignments {
            vertex.packed_uid.set(Some(uid));
        }
        drop(old);
        let old_charge = { live.native_field_charge.write().replace(field_charge) };
        live.opaque_native_fields.set(opaque_fields);
        drop(old_charge);
        drop(charge);
        Ok(())
    }
    pub fn apply(self, live: &PyRef<Mesh>, state: uuid::Uuid) -> Result<(), String> {
        let Self {
            mesh,
            keys,
            assignments,
            charge,
            mut field_charge,
            opaque_fields,
        } = self;
        field_charge.reclassify(Kind::History)?;
        let result = {
            let mut owner = live.owner.write();
            owner
                .prepare_identity_replacement_as(mesh, keys, state)
                .map(|ticket| ticket.apply())
        };
        let old = result?;
        for (vertex, uid) in assignments {
            vertex.packed_uid.set(Some(uid));
        }
        drop(old);
        let old_charge = { live.native_field_charge.write().replace(field_charge) };
        live.opaque_native_fields.set(opaque_fields);
        drop(old_charge);
        drop(charge);
        Ok(())
    }
}
pub(super) fn prepare(
    mesh: &PyRef<Mesh>,
    public: &[PyObjectRef],
    graph: &GraphState,
    positions: &[VertexRestore],
    faces: &[PyObjectRef],
    fields: &[FaceRestore],
    budget: &OwnerBudget,
    vm: &VirtualMachine,
) -> Result<Candidate, String> {
    if faces.len() != fields.len() {
        return Err("Packed Face snapshot extent mismatch".into());
    }
    let mut field_charge = budget.reserve(Kind::Preparation, 0)?;
    let mut opaque_fields = 0usize;
    let registry = graph
        .registry
        .as_ref()
        .ok_or("Missing public registry snapshot")?;
    let mut bound = public
        .len()
        .checked_add(
            graph
                .members
                .len()
                .checked_mul(2)
                .ok_or("Packed endpoint overflow")?,
        )
        .and_then(|n| n.checked_add(registry.__len__()))
        .ok_or("Packed closure extent overflow")?;
    for field in fields {
        bound = bound
            .checked_add(field.outer.borrow_vec().len())
            .ok_or("Packed ring extent overflow")?;
        let holes = field.holes.borrow_vec();
        for ring in holes.iter() {
            let ring = ring
                .downcast_ref::<PyList>()
                .ok_or("Packed hole requires list")?;
            bound = bound
                .checked_add(ring.borrow_vec().len())
                .ok_or("Packed hole extent overflow")?;
        }
    }
    let mut charge = budget.reserve(
        Kind::Preparation,
        bound
            .checked_mul(256)
            .and_then(|n| fields.len().checked_mul(128).and_then(|m| n.checked_add(m)))
            .ok_or("Packed preparation byte overflow")?,
    )?;
    let mut vertices = Vec::new();
    vertices
        .try_reserve_exact(bound)
        .map_err(|_| "Packed closure references refused")?;
    let mut ids = HashMap::new();
    ids.try_reserve(bound)
        .map_err(|_| "Packed closure identity map refused")?;
    fn add(
        value: PyObjectRef,
        ids: &mut HashMap<usize, usize>,
        vertices: &mut Vec<PyObjectRef>,
    ) -> Result<(), String> {
        if value.downcast_ref::<Vertex>().is_none() {
            return Err("Packed closure requires genuine Vertex".into());
        }
        if !ids.contains_key(&value.get_id()) {
            let id = vertices.len();
            ids.insert(value.get_id(), id);
            vertices.push(value);
        }
        Ok(())
    }
    for value in public {
        add(value.clone(), &mut ids, &mut vertices)?;
    }
    if vertices.len() != public.len() {
        return Err("Repeated public Vertex membership unsupported".into());
    }
    for value in &graph.members {
        let edge = value
            .downcast_ref::<super::editable_mesh::Edge>()
            .ok_or("Packed graph member requires Edge")?;
        let a = { edge.v0.read().clone().ok_or("Cleared packed endpoint")? };
        let b = { edge.v1.read().clone().ok_or("Cleared packed endpoint")? };
        add(a.into(), &mut ids, &mut vertices)?;
        add(b.into(), &mut ids, &mut vertices)?;
    }
    for field in fields {
        for value in field.outer.borrow_vec().iter() {
            add(value.clone(), &mut ids, &mut vertices)?;
        }
        for hole in field.holes.borrow_vec().iter() {
            let ring = hole
                .downcast_ref::<PyList>()
                .ok_or("Packed hole requires list")?;
            for value in ring.borrow_vec().iter() {
                add(value.clone(), &mut ids, &mut vertices)?;
            }
        }
    }
    for (_, value) in registry {
        if value.downcast_ref::<Vertex>().is_some() {
            add(value, &mut ids, &mut vertices)?;
        }
    }
    // Public positions are command-owned; detached positions remain current, as
    // source snapshots only include public mesh.vertices, not referenced orphans.
    let mut old_positions = HashMap::new();
    old_positions
        .try_reserve(positions.len())
        .map_err(|_| "Packed snapshot position map refused")?;
    for p in positions {
        old_positions.insert(p.vertex.get_id(), p.position.coordinates());
    }
    let mut assignments = Vec::new();
    assignments
        .try_reserve_exact(vertices.len())
        .map_err(|_| "Packed UID assignments refused")?;
    let mut out = ingetrazo_core::mesh::Mesh::new();
    out.vertices
        .try_reserve_exact(vertices.len())
        .map_err(|_| "Packed Vertex records refused")?;
    for value in &vertices {
        let vertex = value
            .clone()
            .downcast::<Vertex>()
            .map_err(|_| "Packed Vertex identity invalid")?;
        let p = old_positions
            .get(&vertex.get_id())
            .copied()
            .unwrap_or_else(|| vertex.position.read().coordinates());
        let position = glam::DVec3::from_array(p.map(f64::from));
        if !position.is_finite() {
            return Err("Nonfinite packed Vertex".into());
        }
        let uid = vertex.packed_uid.get().unwrap_or_else(uuid::Uuid::new_v4);
        out.vertices.push(ingetrazo_core::mesh::MeshVertex {
            uid,
            position,
            edges: Vec::new(),
        });
        assignments.push((vertex, uid));
    }
    let (edges, edge_charge) = graph.packed_edges(&vertices, faces, budget, vm)?;
    charge.merge(edge_charge)?;
    out.faces
        .try_reserve_exact(fields.len())
        .map_err(|_| "Packed Face records refused")?;
    let ring = |values: &rustpython_vm::builtins::PyListRef| -> Result<Vec<usize>, String> {
        let values = values.borrow_vec();
        let mut result = Vec::new();
        result
            .try_reserve_exact(values.len())
            .map_err(|_| "Packed ring indices refused")?;
        for value in values.iter() {
            result.push(
                *ids.get(&value.get_id())
                    .ok_or("Packed ring Vertex missing")?,
            );
        }
        Ok(result)
    };
    for field in fields {
        let outer = ring(&field.outer)?;
        let holes = field.holes.borrow_vec();
        let mut rings = Vec::new();
        rings
            .try_reserve_exact(holes.len())
            .map_err(|_| "Packed hole slots refused")?;
        for hole in holes.iter() {
            rings.push(ring(
                &hole
                    .clone()
                    .downcast::<PyList>()
                    .map_err(|_| "Packed hole requires list")?,
            )?);
        }
        let mut face = ingetrazo_core::mesh::MeshFace::new(outer);
        face.holes = rings;
        let fields = super::face_fields::prepare(&field.attrs, budget, vm)?;
        face.attrs = fields.attrs;
        face.layer = fields.layer;
        opaque_fields = opaque_fields
            .checked_add(fields.opaque)
            .ok_or("Opaque native Face count overflow")?;
        field_charge.merge(fields.charge)?;
        out.faces.push(face);
    }
    let mut counts = Vec::new();
    counts
        .try_reserve_exact(vertices.len())
        .map_err(|_| "Packed incidence counts refused")?;
    counts.resize(vertices.len(), 0usize);
    for edge in &edges {
        counts[edge.v0] = counts[edge.v0]
            .checked_add(1)
            .ok_or("Packed incidence overflow")?;
        if edge.v1 != edge.v0 {
            counts[edge.v1] = counts[edge.v1]
                .checked_add(1)
                .ok_or("Packed incidence overflow")?;
        }
    }
    let links = counts
        .iter()
        .try_fold(0usize, |n, &m| n.checked_add(m))
        .ok_or("Packed link extent overflow")?;
    charge.resize(
        charge
            .bytes()
            .checked_add(
                links
                    .checked_mul(std::mem::size_of::<usize>())
                    .ok_or("Packed link bytes overflow")?,
            )
            .ok_or("Packed link charge overflow")?,
    )?;
    for (vertex, count) in out.vertices.iter_mut().zip(counts) {
        vertex
            .edges
            .try_reserve_exact(count)
            .map_err(|_| "Packed incidence allocation refused")?;
    }
    for (id, edge) in edges.iter().enumerate() {
        out.vertices[edge.v0].edges.push(id);
        if edge.v0 != edge.v1 {
            out.vertices[edge.v1].edges.push(id);
        }
    }
    out.edges = edges;
    let mut keys = Vec::new();
    keys.try_reserve_exact(registry.__len__())
        .map_err(|_| "Packed registry rows refused")?;
    for (key, value) in registry {
        // The actual dictionary is retained verbatim. This derived numeric cache
        // admits only literal integer-cell -> Vertex pairs; other Python entries
        // remain in VM/history and input lookup admission detects cache disagreement.
        let Some(tuple) = key.downcast_ref::<PyTuple>() else {
            continue;
        };
        if tuple.as_slice().len() != 3
            || tuple
                .as_slice()
                .iter()
                .any(|c| c.downcast_ref::<PyInt>().is_none())
            || value.downcast_ref::<Vertex>().is_none()
        {
            continue;
        }
        let mut cells = Vec::new();
        cells
            .try_reserve_exact(3)
            .map_err(|_| "Packed registry cells refused")?;
        for cell in tuple.as_slice() {
            let n = cell.downcast_ref::<PyInt>().unwrap();
            cells.push(
                n.as_bigint()
                    .to_string()
                    .parse::<num_bigint::BigInt>()
                    .map_err(|_| "Packed registry integer conversion refused")?,
            );
        }
        let id = *ids
            .get(&value.get_id())
            .ok_or("Native registry Vertex closure missing")?;
        keys.push((
            SourceKey(
                cells
                    .try_into()
                    .map_err(|_| "Packed registry cell extent")?,
            ),
            id,
        ));
    }
    // Packed rows bypass Mesh::add_vertex. Prepare the native derived lookup
    // before publication: Mesh semantic equality includes that cache, and a
    // read-only import must not replace a coherent cache with an empty one.
    // This is separate from the genuine Python registry retained in `keys`.
    let index_bytes = out
        .vertices
        .len()
        .checked_mul(160)
        .ok_or("Packed native index extent overflow")?;
    field_charge.merge(budget.reserve(Kind::Preparation, index_bytes)?)?;
    let mut index_positions = Vec::new();
    index_positions
        .try_reserve_exact(out.vertices.len())
        .map_err(|_| "Packed native index coordinates refused")?;
    index_positions.extend(out.vertices.iter().map(|vertex| vertex.position));
    let index = ingetrazo_core::mesh::Mesh::prepare_spatial_index(&index_positions)
        .map_err(str::to_owned)?;
    out.install_prepared_spatial_index(index)
        .map_err(str::to_owned)?;
    drop(index_positions);
    let native = mesh.owner.read();
    if native.mesh().faces.iter().any(|face| face.interior) {
        return Err("Native painted Face closure admission pending".into());
    }
    Ok(Candidate {
        mesh: out,
        keys,
        assignments,
        charge,
        field_charge,
        opaque_fields,
    })
}

/// Cold command-owned ID table. No coordinate rebinding: every native UID must
/// resolve to an actual retained object before append staging can be published.
pub(super) fn references(
    mesh: &PyRef<Mesh>,
    graph: &GraphState,
    budget: &OwnerBudget,
) -> Result<(Vec<PyObjectRef>, Lease), String> {
    let count = mesh.owner.read().mesh().vertices.len();
    let charge = budget.reserve(
        Kind::Staging,
        count
            .checked_mul(256)
            .ok_or("Packed reference table overflow")?,
    )?;
    let mut wanted = std::collections::HashSet::new();
    wanted
        .try_reserve(count)
        .map_err(|_| "Packed native UID admission refused")?;
    {
        let owner = mesh.owner.read();
        for vertex in &owner.mesh().vertices {
            if !wanted.insert(vertex.uid) {
                return Err("Repeated native packed UID".into());
            }
        }
    }
    let mut by_uid: HashMap<uuid::Uuid, PyObjectRef> = HashMap::new();
    by_uid
        .try_reserve(count)
        .map_err(|_| "Packed reference identities refused")?;
    let mut remember = |value: PyObjectRef| -> Result<(), String> {
        if let Some(v) = value.downcast_ref::<Vertex>() {
            if let Some(uid) = v.packed_uid.get() {
                if !wanted.contains(&uid) {
                    return Ok(());
                }
                if let Some(old) = by_uid.get(&uid) {
                    if !AsObject::is(old, &value) {
                        return Err("Packed UID has multiple genuine owners".into());
                    }
                } else {
                    by_uid.insert(uid, value);
                }
            }
        }
        Ok(())
    };
    {
        let values = mesh.vertices.borrow_vec();
        for value in values.iter() {
            remember(value.clone())?;
        }
    }
    for value in &graph.members {
        let edge = value
            .downcast_ref::<super::editable_mesh::Edge>()
            .ok_or("Packed graph requires Edge")?;
        let a = { edge.v0.read().clone().ok_or("Cleared endpoint")? };
        let b = { edge.v1.read().clone().ok_or("Cleared endpoint")? };
        remember(a.into())?;
        remember(b.into())?;
    }
    {
        let faces = mesh.faces.borrow_vec();
        for value in faces.iter() {
            let face = value
                .downcast_ref::<super::editable_mesh::Face>()
                .ok_or("Packed Face identity")?;
            let outer = face.r#loop.read().clone();
            for value in outer.borrow_vec().iter() {
                remember(value.clone())?;
            }
            let holes = face.hole_loops.read().clone();
            for hole in holes.borrow_vec().iter() {
                let ring = hole
                    .downcast_ref::<PyList>()
                    .ok_or("Packed hole requires list")?;
                for value in ring.borrow_vec().iter() {
                    remember(value.clone())?;
                }
            }
        }
    }
    if let Some(registry) = &graph.registry {
        for (_, value) in registry {
            remember(value)?;
        }
    }
    let mut result = Vec::new();
    result
        .try_reserve_exact(count)
        .map_err(|_| "Packed reference rows refused")?;
    {
        let owner = mesh.owner.read();
        for vertex in &owner.mesh().vertices {
            result.push(
                by_uid
                    .remove(&vertex.uid)
                    .ok_or("Packed row lacks genuine retained reference")?,
            );
        }
    }
    Ok((result, charge))
}
