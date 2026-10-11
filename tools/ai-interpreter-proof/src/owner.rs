//! Private append owner: state tokens replace per-command base snapshots.
use crate::journal::{validate_base, MeshJournal};
use crate::owner_budget::{Kind, Lease, OwnerBudget};
use crate::owner_registry::Registry;
use glam::DVec3;
use ingetrazo_core::mesh::{
    Mesh, MeshEdge, MeshVertex, PreparedSpatialDelta, PreparedSpatialIndex,
};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;
pub struct Transaction {
    pub(crate) force_command: bool,
    document: Uuid,
    state: Uuid,
    pub(crate) journal: MeshJournal,
    charge: Lease,
    admission_failed: bool,
}
impl Transaction {
    pub fn registry_lookup_calls(&self) -> usize {
        self.journal.registry.as_ref().unwrap().lookup_calls()
    }
    pub fn add_edge(&mut self, a: DVec3, b: DVec3) -> Result<(), String> {
        if self.admission_failed {
            return Err("Transaction shared admission previously refused".into());
        }
        let next = staging_bound(
            0,
            0,
            self.journal
                .added_vertices
                .len()
                .checked_add(2)
                .ok_or("Staging extent overflow")?,
            self.journal
                .added_edges
                .len()
                .checked_add(1)
                .ok_or("Staging extent overflow")?,
        )?;
        if let Err(error) = self.charge.resize(next) {
            self.admission_failed = true;
            return Err(error);
        }
        let result = self.journal.add_edge(a, b).map(|_| ());
        self.charge.resize(staging_bound(
            0,
            0,
            self.journal.added_vertices.len(),
            self.journal.added_edges.len(),
        )?)?;
        result
    }
}
struct MixedPosition {
    id: usize,
    before: DVec3,
    after: DVec3,
}
struct MixedLoop {
    id: usize,
    parked: (Vec<usize>, Vec<Vec<usize>>),
}
struct MixedFields {
    positions: Vec<MixedPosition>,
    loops: Vec<MixedLoop>,
    _charge: Lease,
}
impl MixedFields {
    fn apply(&mut self, mesh: &mut Mesh, forward: bool) {
        for p in &self.positions {
            mesh.vertices[p.id].position = if forward { p.after } else { p.before };
        }
        for f in &mut self.loops {
            std::mem::swap(&mut mesh.faces[f.id].vertices, &mut f.parked.0);
            std::mem::swap(&mut mesh.faces[f.id].holes, &mut f.parked.1);
        }
    }
    fn positions(&self, mesh: &Mesh, forward: bool) -> Result<Vec<DVec3>, String> {
        let mut out = positions(&mesh.vertices, 0)?;
        for p in &self.positions {
            out[p.id] = if forward { p.after } else { p.before };
        }
        Ok(out)
    }
}
struct Delta {
    before: Uuid,
    after: Uuid,
    base_vertices: usize,
    base_edges: usize,
    vertices: Vec<MeshVertex>,
    edges: Vec<MeshEdge>,
    bytes: usize,
    charge: Lease,
    spatial: PreparedSpatialDelta,
    mixed: Option<MixedFields>,
}
pub struct JournalOwner {
    mesh: Mesh,
    document: Uuid,
    state: Uuid,
    version: u64,
    undo: Vec<Delta>,
    redo: Vec<Delta>,
    index_entry_limit: Option<usize>,
    history_byte_limit: Option<usize>,
    undo_bytes: usize,
    redo_bytes: usize,
    append_vertex_limit: Option<usize>,
    append_edge_limit: Option<usize>,
    budget: OwnerBudget,
    registry: Registry,
}
/// Exclusive prepared publication. Dropping it discards the unpublished delta;
/// holding it prevents any owner mutation between admission and application.
pub struct PreparedAppend<'a> {
    owner: &'a mut JournalOwner,
    vertices: Vec<MeshVertex>,
    edges: Vec<MeshEdge>,
    additions: HashMap<usize, Vec<usize>>,
    delta: Delta,
    registry_change: crate::owner_registry::Change,
    base_vertices: usize,
    after: Uuid,
    next_version: u64,
    projected: usize,
    final_spatial: Option<PreparedSpatialIndex>,
    _mixed_preparation: Option<Lease>,
    _preparation: Lease,
    _staging: Lease,
}
impl<'a> PreparedAppend<'a> {
    /// Prepare all existing-field changes before the append ticket publishes.
    /// Raw loop edits intentionally do not rebuild the source radial graph.
    pub fn with_existing_fields(
        mut self,
        positions: Vec<DVec3>,
        loops: Vec<(Vec<usize>, Vec<Vec<usize>>)>,
    ) -> Result<Self, String> {
        if positions.len() != self.base_vertices || loops.len() != self.owner.mesh.faces.len() {
            return Err("Mixed base membership mismatch".into());
        }
        let total = self
            .base_vertices
            .checked_add(self.vertices.len())
            .ok_or("Mixed extent overflow")?;
        for (outer, holes) in &loops {
            if outer
                .iter()
                .chain(holes.iter().flatten())
                .any(|&id| id >= total)
            {
                return Err("Mixed loop vertex outside final membership".into());
            }
        }
        let preparation = self
            .owner
            .budget
            .reserve(Kind::Preparation, preparation_bound(total, 0)?)?;
        let npos = positions
            .iter()
            .zip(&self.owner.mesh.vertices)
            .filter(|(p, v)| **p != v.position)
            .count();
        let nloop = loops
            .iter()
            .zip(&self.owner.mesh.faces)
            .filter(|((o, h), f)| *o != f.vertices || *h != f.holes)
            .count();
        let mut bytes = sized(npos, std::mem::size_of::<MixedPosition>())?
            .checked_add(sized(nloop, std::mem::size_of::<MixedLoop>())?)
            .ok_or("Mixed history overflow")?;
        for ((outer, holes), face) in loops.iter().zip(&self.owner.mesh.faces) {
            if *outer != face.vertices || *holes != face.holes {
                for ring in std::iter::once(outer).chain(holes) {
                    bytes = add(bytes, sized(ring.capacity(), std::mem::size_of::<usize>())?)?;
                }
                for ring in std::iter::once(&face.vertices).chain(&face.holes) {
                    bytes = add(bytes, sized(ring.capacity(), std::mem::size_of::<usize>())?)?;
                }
                bytes = add(
                    bytes,
                    sized(
                        holes.capacity() + face.holes.capacity(),
                        std::mem::size_of::<Vec<usize>>(),
                    )?,
                )?;
            }
        }
        let charge = self.owner.budget.reserve(Kind::History, bytes)?;
        let mut patch = Vec::new();
        patch
            .try_reserve_exact(npos)
            .map_err(|_| "Mixed position history refused")?;
        for (id, (p, v)) in positions.iter().zip(&self.owner.mesh.vertices).enumerate() {
            if *p != v.position {
                patch.push(MixedPosition {
                    id,
                    before: v.position,
                    after: *p,
                });
            }
        }
        let mut fields = Vec::new();
        fields
            .try_reserve_exact(nloop)
            .map_err(|_| "Mixed loop history refused")?;
        for (id, ((outer, holes), face)) in
            loops.into_iter().zip(&self.owner.mesh.faces).enumerate()
        {
            if outer != face.vertices || holes != face.holes {
                fields.push(MixedLoop {
                    id,
                    parked: (outer, holes),
                });
            }
        }
        let registry = self.owner.registry.prepare_append_positions(
            &self.vertices,
            &self.edges,
            &positions,
        )?;
        let mut final_positions = positions;
        final_positions
            .try_reserve_exact(self.vertices.len())
            .map_err(|_| "Mixed final position preparation refused")?;
        final_positions.extend(self.vertices.iter().map(|v| v.position));
        let index = self.owner.index(&final_positions)?;
        let projected = add(self.projected, bytes)?;
        if self
            .owner
            .history_byte_limit
            .is_some_and(|limit| projected > limit)
        {
            return Err("Mixed history payload admission denied".into());
        }
        self.delta.bytes = add(self.delta.bytes, bytes)?;
        self.delta.mixed = Some(MixedFields {
            positions: patch,
            loops: fields,
            _charge: charge,
        });
        self.projected = projected;
        self.registry_change = registry;
        self.final_spatial = Some(index);
        self._mixed_preparation = Some(preparation);
        Ok(self)
    }
    pub fn apply(self) -> &'a mut JournalOwner {
        let Self {
            owner,
            vertices,
            edges,
            additions,
            mut delta,
            registry_change,
            base_vertices,
            after,
            next_version,
            projected,
            final_spatial,
            _mixed_preparation,
            _preparation,
            _staging,
        } = self;
        owner.mesh.vertices.extend(vertices);
        owner.mesh.edges.extend(edges);
        for (id, ids) in additions {
            if id < base_vertices {
                owner.mesh.vertices[id].edges.extend(ids);
            }
        }
        if let Some(fields) = &mut delta.mixed {
            fields.apply(&mut owner.mesh, true);
        }
        if let Some(index) = final_spatial {
            owner
                .mesh
                .install_prepared_spatial_index(index)
                .expect("exclusive mixed final positions");
        } else {
            owner
                .mesh
                .install_spatial_append(&delta.spatial)
                .expect("exclusive prepared append positions");
        }
        owner.state = after;
        owner.version = next_version;
        registry_change
            .apply(after, next_version)
            .expect("exclusive prepared registry append");
        owner.redo.clear();
        owner.redo_bytes = 0;
        owner.undo_bytes = projected;
        owner.undo.push(delta);
        drop(_preparation);
        drop(_staging);
        owner
    }
}
impl JournalOwner {
    pub fn new(mesh: Mesh) -> Result<Self, String> {
        Self::new_with_budget(mesh, OwnerBudget::new(None))
    }
    pub fn new_with_budget(mut mesh: Mesh, budget: OwnerBudget) -> Result<Self, String> {
        let _preparation = budget.reserve(
            Kind::Preparation,
            preparation_bound(mesh.vertices.len(), mesh.edges.len())?,
        )?;
        validate_base(&mesh)?;
        let positions = positions(&mesh.vertices, 0)?;
        let index = Mesh::prepare_spatial_index(&positions).map_err(str::to_owned)?;
        mesh.install_prepared_spatial_index(index)
            .map_err(str::to_owned)?;
        let document = Uuid::new_v4();
        let state = Uuid::new_v4();
        let registry = Registry::new(&mesh, budget.clone(), document, state, 0)?;
        Ok(Self {
            mesh,
            document,
            state,
            version: 0,
            undo: vec![],
            redo: vec![],
            index_entry_limit: None,
            history_byte_limit: None,
            undo_bytes: 0,
            redo_bytes: 0,
            append_vertex_limit: None,
            append_edge_limit: None,
            budget: budget.clone(),
            registry,
        })
    }
    pub fn registry_metrics(&self) -> crate::owner_registry::Metrics {
        self.registry.metrics()
    }
    pub fn registry_identity(&self) -> usize {
        self.registry.identity()
    }
    pub fn budget(&self) -> OwnerBudget {
        self.budget.clone()
    }
    pub fn mesh(&self) -> &Mesh {
        &self.mesh
    }
    pub(crate) fn state_token(&self) -> Uuid {
        self.state
    }
    /// Consume a private newly constructed owner without cloning native geometry.
    pub(crate) fn into_mesh(self) -> Mesh {
        self.mesh
    }
    pub(crate) fn face_storage(
        &mut self,
    ) -> (&mut Vec<ingetrazo_core::mesh::MeshFace>, &mut Vec<MeshEdge>) {
        (&mut self.mesh.faces, &mut self.mesh.edges)
    }
    pub fn history_counts(&self) -> (usize, usize) {
        (self.undo.len(), self.redo.len())
    }
    pub fn retained_baseline_entries(&self) -> usize {
        0
    }
    /// Engineering/host admission quota, not a simulated allocator error.
    pub fn set_index_entry_limit(&mut self, limit: Option<usize>) {
        self.index_entry_limit = limit;
    }
    /// Retained delta payload admission, excluding VM, scratch and history Vec slack.
    pub fn set_history_byte_limit(&mut self, limit: Option<usize>) {
        self.history_byte_limit = limit;
    }
    pub fn history_payload_bytes(&self) -> (usize, usize) {
        (self.undo_bytes, self.redo_bytes)
    }
    pub fn set_append_limits(&mut self, vertices: Option<usize>, edges: Option<usize>) {
        self.append_vertex_limit = vertices;
        self.append_edge_limit = edges;
    }
    pub fn begin(&self) -> Transaction {
        self.try_begin().expect("legacy proof owner preparation")
    }
    pub fn try_begin(&self) -> Result<Transaction, String> {
        let charge = self
            .budget
            .reserve(Kind::Staging, staging_bound(0, 0, 0, 0)?)?;
        let mut journal = MeshJournal::from_registry(self.registry.view(), self.version)?;
        journal.set_append_limits(self.append_vertex_limit, self.append_edge_limit);
        Ok(Transaction {
            force_command: false,
            document: self.document,
            state: self.state,
            journal,
            charge,
            admission_failed: false,
        })
    }
    fn index(&self, positions: &[DVec3]) -> Result<PreparedSpatialIndex, String> {
        if self.index_entry_limit.is_some_and(|n| positions.len() > n) {
            return Err("Prepared-index entry admission denied".into());
        }
        Mesh::prepare_spatial_index(positions).map_err(str::to_owned)
    }
    /// Cold packed representation replacement prerequisite. The caller owns the
    /// source identity registry and inverse command, plus candidate payload
    /// admission. This does not implement source deletion/lookup by itself.
    /// Existing chronological append deltas remain parked until inverse restores
    /// their state token/packing. No public production scripting route uses it.
    pub fn prepare_packed_replacement_as(&mut self,mesh:Mesh,state:Uuid)->Result<PreparedPackedReplacement<'_>,String>{self.prepare_packed_replacement_inner(mesh,state,false,None)}
    pub fn prepare_edge_replacement_as(&mut self,mesh:Mesh,state:Uuid)->Result<PreparedPackedReplacement<'_>,String>{
        if mesh.vertices.len()!=self.mesh.vertices.len()||mesh.vertices.iter().zip(&self.mesh.vertices).any(|(a,b)|a.uid!=b.uid){return Err("Edge replacement requires exact Vertex identity order".into());}
        self.prepare_packed_replacement_inner(mesh,state,true,None)
    }
    pub fn prepare_identity_history_replacement_as(&mut self,mesh:Mesh,keys:Vec<(crate::journal::SourceKey,usize)>,state:Uuid,redo:bool)->Result<PreparedPackedReplacement<'_>,String>{
        let command=if redo{self.redo.last()}else{self.undo.last()}.ok_or("Packed history command missing")?;
        if self.state!=if redo{command.before}else{command.after}||state!=if redo{command.after}else{command.before}{return Err("Packed history state mismatch".into());}
        if redo{self.undo.try_reserve(1).map_err(|_|"Packed undo destination refused")?;}else{self.redo.try_reserve(1).map_err(|_|"Packed redo destination refused")?;}
        let mut ticket=self.prepare_identity_replacement_as(mesh,keys,state)?;ticket.history=Some(redo);Ok(ticket)
    }
    pub fn prepare_identity_replacement_as(&mut self,mesh:Mesh,keys:Vec<(crate::journal::SourceKey,usize)>,state:Uuid)->Result<PreparedPackedReplacement<'_>,String>{self.prepare_packed_replacement_inner(mesh,state,false,Some(keys))}
    fn prepare_packed_replacement_inner(&mut self,mut mesh:Mesh,state:Uuid,keys:bool,owned_keys:Option<Vec<(crate::journal::SourceKey,usize)>>)->Result<PreparedPackedReplacement<'_>,String>{
        let next_version=self.version.checked_add(1).ok_or("Owner revision overflow")?;
        let preparation=self.budget.reserve(Kind::Preparation,preparation_bound(mesh.vertices.len(),mesh.edges.len())?)?;
        validate_base(&mesh)?;
        let positions=positions(&mesh.vertices,0)?;
        let index=self.index(&positions)?;
        mesh.install_prepared_spatial_index(index).map_err(str::to_owned)?;
        let registry=if let Some(rows)=owned_keys{self.registry.identity_replacement(&mesh,rows,state,next_version)?}else if keys{self.registry.edge_replacement(&mesh,state,next_version)?}else{Registry::new(&mesh,self.budget.clone(),self.document,state,next_version)?};
        Ok(PreparedPackedReplacement{owner:self,mesh,registry,state,next_version,_preparation:preparation,history:None})
    }
    pub fn replace(&mut self, mut mesh: Mesh) -> Result<(), String> {
        let next_version = self
            .version
            .checked_add(1)
            .ok_or("Owner revision overflow")?;
        let _preparation = self.budget.reserve(
            Kind::Preparation,
            preparation_bound(mesh.vertices.len(), mesh.edges.len())?,
        )?;
        validate_base(&mesh)?;
        let positions = positions(&mesh.vertices, 0)?;
        let index = self.index(&positions)?;
        mesh.install_prepared_spatial_index(index)
            .map_err(str::to_owned)?;
        let document = Uuid::new_v4();
        let state = Uuid::new_v4();
        let registry = Registry::new(&mesh, self.budget.clone(), document, state, next_version)?;
        self.registry.invalidate();
        self.registry = registry;
        self.mesh = mesh;
        self.document = document;
        self.state = state;
        self.version = next_version;
        self.undo.clear();
        self.redo.clear();
        self.undo_bytes = 0;
        self.redo_bytes = 0;
        Ok(())
    }
    pub fn commit(&mut self, t: Transaction) -> Result<bool, String> {
        let Some(prepared) = self.prepare(t)? else {
            return Ok(false);
        };
        prepared.apply();
        Ok(true)
    }
    pub fn prepare(&mut self, t: Transaction) -> Result<Option<PreparedAppend<'_>>, String> {
        if t.document != self.document
            || t.state != self.state
            || t.journal.base_version != self.version
        {
            return Err("Stale owner transaction".into());
        }
        if t.admission_failed {
            return Err("Transaction shared staging admission failed".into());
        }
        let next_version = self
            .version
            .checked_add(1)
            .ok_or("Owner revision overflow")?;
        let force = t.force_command;
        let j = t.journal;
        let _staging = t.charge;
        if j.admission_failed {
            return Err("Transaction staging admission failed".into());
        }
        if !force && j.added_vertices.is_empty() && j.added_edges.is_empty() {
            return Ok(None);
        }
        let base_vertices = self.mesh.vertices.len();
        let base_edges = self.mesh.edges.len();
        let total = base_vertices
            .checked_add(j.added_vertices.len())
            .ok_or("Vertex extent overflow")?;
        let edge_total = base_edges
            .checked_add(j.added_edges.len())
            .ok_or("Edge extent overflow")?;
        let _preparation = self.budget.reserve(
            Kind::Preparation,
            preparation_bound(j.added_vertices.len(), j.added_edges.len())?,
        )?;
        let mut history_charge = self.budget.reserve(
            Kind::History,
            delta_bound(j.added_vertices.len(), j.added_edges.len())?,
        )?;
        let mut vertices = Vec::new();
        vertices
            .try_reserve(j.added_vertices.len())
            .map_err(|_| "Vertex preparation refused")?;
        for p in j.added_vertices {
            vertices.push(MeshVertex::new(p));
        }
        let mut edges = Vec::new();
        edges
            .try_reserve(j.added_edges.len())
            .map_err(|_| "Edge preparation refused")?;
        let mut additions = HashMap::<usize, Vec<usize>>::new();
        additions
            .try_reserve(
                j.added_edges
                    .len()
                    .checked_mul(2)
                    .ok_or("Adjacency extent overflow")?,
            )
            .map_err(|_| "Adjacency preparation refused")?;
        for (offset, (a, b)) in j.added_edges.into_iter().enumerate() {
            if a >= total || b >= total || (!force && a == b) {
                return Err("Invalid planned endpoint".into());
            }
            let id = base_edges
                .checked_add(offset)
                .ok_or("Edge extent overflow")?;
            for endpoint in [a, b].into_iter().take(if a == b { 1 } else { 2 }) {
                let entries = additions.entry(endpoint).or_default();
                entries
                    .try_reserve(1)
                    .map_err(|_| "Adjacency preparation refused")?;
                entries.push(id);
            }
            edges.push(MeshEdge::new(a, b));
        }
        for (&id, ids) in &additions {
            if id >= base_vertices {
                let adjacency = &mut vertices[id - base_vertices].edges;
                adjacency
                    .try_reserve_exact(ids.len())
                    .map_err(|_| "Prepared vertex adjacency refused")?;
                adjacency.extend(ids);
            }
        }
        if self.index_entry_limit.is_some_and(|n| total > n) {
            return Err("Prepared-index entry admission denied".into());
        }
        let appended_positions = positions(&vertices, 0)?;
        let spatial = self
            .mesh
            .prepare_spatial_append(&appended_positions)
            .map_err(str::to_owned)?;
        // Delta copies concern only new records. No base UUID/position/edge arrays
        // survive the transaction, and no Scene/Mesh snapshot is retained.
        let after = Uuid::new_v4();
        let retained_vertices = copy_vertices(&vertices)?;
        let retained_edges = copy_append_edges(&edges)?;
        let bytes = delta_bytes(&retained_vertices, &retained_edges)?
            .checked_add(spatial.retained_bytes())
            .ok_or("Spatial delta byte overflow")?;
        history_charge.resize(bytes)?;
        let projected = self
            .undo_bytes
            .checked_add(bytes)
            .ok_or("History payload extent overflow")?;
        if self
            .history_byte_limit
            .is_some_and(|limit| projected > limit)
        {
            return Err("History payload admission denied".into());
        }
        let delta = Delta {
            before: self.state,
            after,
            base_vertices,
            base_edges,
            vertices: retained_vertices,
            edges: retained_edges,
            bytes,
            charge: history_charge,
            spatial,
            mixed: None,
        };
        self.undo
            .try_reserve(1)
            .map_err(|_| "History admission refused")?;
        self.mesh
            .vertices
            .try_reserve(vertices.len())
            .map_err(|_| "Vertex admission refused")?;
        self.mesh
            .edges
            .try_reserve(edges.len())
            .map_err(|_| "Edge admission refused")?;
        for (&id, ids) in &additions {
            if id < base_vertices {
                self.mesh.vertices[id]
                    .edges
                    .try_reserve(ids.len())
                    .map_err(|_| "Live adjacency admission refused")?;
            }
        }
        let registry_change = self.registry.prepare_append(&vertices, &edges)?;
        Ok(Some(PreparedAppend {
            owner: self,
            vertices,
            edges,
            additions,
            delta,
            registry_change,
            base_vertices,
            after,
            next_version,
            projected,
            final_spatial: None,
            _mixed_preparation: None,
            _preparation,
            _staging,
        }))
    }
    pub fn undo(&mut self) -> Result<bool, String> {
        let Some(d) = self.undo.last() else {
            return Ok(false);
        };
        if self.state != d.after {
            return Err("Stale undo state".into());
        }
        let next_version = self
            .version
            .checked_add(1)
            .ok_or("Owner revision overflow")?;
        let _preparation = self.budget.reserve(
            Kind::Preparation,
            preparation_bound(d.vertices.len(), d.edges.len())?,
        )?;
        if self.index_entry_limit.is_some_and(|n| d.base_vertices > n) {
            return Err("Prepared-index entry admission denied".into());
        }
        self.redo
            .try_reserve(1)
            .map_err(|_| "Redo history admission refused")?;
        let mut touched = HashSet::new();
        touched
            .try_reserve(
                d.edges
                    .len()
                    .checked_mul(2)
                    .ok_or("Touched extent overflow")?,
            )
            .map_err(|_| "Touched endpoint admission refused")?;
        for e in &d.edges {
            for id in [e.v0, e.v1] {
                if id < d.base_vertices {
                    touched.insert(id);
                }
            }
        }
        let mixed_preparation = if d.mixed.is_some() {
            Some(self.budget.reserve(
                Kind::Preparation,
                preparation_bound(self.mesh.vertices.len(), 0)?,
            )?)
        } else {
            None
        };
        let (registry_change, index) = if let Some(fields) = &d.mixed {
            let positions = fields.positions(&self.mesh, false)?;
            let index = self.index(&positions[..d.base_vertices])?;
            (
                self.registry.prepare_truncate_positions(
                    d.base_vertices,
                    d.base_edges,
                    &d.vertices,
                    &d.edges,
                    &positions,
                )?,
                Some(index),
            )
        } else {
            (
                self.registry.prepare_truncate(
                    d.base_vertices,
                    d.base_edges,
                    &d.vertices,
                    &d.edges,
                )?,
                None,
            )
        };
        if index.is_none() {
            self.mesh
                .restore_spatial_append(&d.spatial)
                .map_err(str::to_owned)?;
        }
        let mut d = self.undo.pop().unwrap();
        self.mesh.edges.truncate(d.base_edges);
        self.mesh.vertices.truncate(d.base_vertices);
        if let Some(fields) = &mut d.mixed {
            fields.apply(&mut self.mesh, false);
        }
        if let Some(index) = index {
            self.mesh
                .install_prepared_spatial_index(index)
                .expect("prepared mixed Undo positions");
        }
        drop(mixed_preparation);
        for id in touched {
            self.mesh.vertices[id].edges.retain(|&e| e < d.base_edges);
        }
        self.state = d.before;
        self.version = next_version;
        registry_change
            .apply(d.before, next_version)
            .expect("private prepared registry undo");
        self.undo_bytes -= d.bytes;
        self.redo_bytes += d.bytes;
        self.redo.push(d);
        Ok(true)
    }
    pub fn redo(&mut self) -> Result<bool, String> {
        let Some(d) = self.redo.last() else {
            return Ok(false);
        };
        if self.state != d.before {
            return Err("Stale redo state".into());
        }
        let next_version = self
            .version
            .checked_add(1)
            .ok_or("Owner revision overflow")?;
        let total = self
            .mesh
            .vertices
            .len()
            .checked_add(d.vertices.len())
            .ok_or("Vertex extent overflow")?;
        let _preparation = self.budget.reserve(
            Kind::Preparation,
            preparation_bound(d.vertices.len(), d.edges.len())?,
        )?;
        if self.index_entry_limit.is_some_and(|n| total > n) {
            return Err("Prepared-index entry admission denied".into());
        }
        if d.mixed.is_none() {
            self.mesh
                .reserve_spatial_append(&d.spatial)
                .map_err(str::to_owned)?;
        }
        let vertices = copy_vertices(&d.vertices)?;
        let edges = copy_append_edges(&d.edges)?;
        let mut additions = HashMap::<usize, Vec<usize>>::new();
        additions
            .try_reserve(
                edges
                    .len()
                    .checked_mul(2)
                    .ok_or("Adjacency extent overflow")?,
            )
            .map_err(|_| "Redo adjacency map refused")?;
        for (offset, e) in edges.iter().enumerate() {
            for id in [e.v0, e.v1]
                .into_iter()
                .take(if e.v0 == e.v1 { 1 } else { 2 })
            {
                if id < d.base_vertices {
                    let ids = additions.entry(id).or_default();
                    ids.try_reserve(1)
                        .map_err(|_| "Redo prepared adjacency refused")?;
                    ids.push(
                        d.base_edges
                            .checked_add(offset)
                            .ok_or("Redo edge extent overflow")?,
                    );
                }
            }
        }
        self.undo
            .try_reserve(1)
            .map_err(|_| "Undo history admission refused")?;
        self.mesh
            .vertices
            .try_reserve(vertices.len())
            .map_err(|_| "Redo vertex admission refused")?;
        self.mesh
            .edges
            .try_reserve(edges.len())
            .map_err(|_| "Redo edge admission refused")?;
        for (&id, ids) in &additions {
            self.mesh.vertices[id]
                .edges
                .try_reserve(ids.len())
                .map_err(|_| "Redo adjacency admission refused")?;
        }
        let mixed_preparation = if d.mixed.is_some() {
            Some(
                self.budget
                    .reserve(Kind::Preparation, preparation_bound(total, 0)?)?,
            )
        } else {
            None
        };
        let (registry_change, index) = if let Some(fields) = &d.mixed {
            let mut positions = fields.positions(&self.mesh, true)?;
            let registry = self
                .registry
                .prepare_append_positions(&vertices, &edges, &positions)?;
            positions
                .try_reserve_exact(vertices.len())
                .map_err(|_| "Mixed Redo positions refused")?;
            positions.extend(vertices.iter().map(|v| v.position));
            (registry, Some(self.index(&positions)?))
        } else {
            (self.registry.prepare_append(&vertices, &edges)?, None)
        };
        let mut d = self.redo.pop().unwrap();
        self.mesh.vertices.extend(vertices);
        self.mesh.edges.extend(edges);
        for (id, ids) in additions {
            self.mesh.vertices[id].edges.extend(ids);
        }
        if let Some(fields) = &mut d.mixed {
            fields.apply(&mut self.mesh, true);
        }
        if let Some(index) = index {
            self.mesh
                .install_prepared_spatial_index(index)
                .expect("prepared mixed Redo positions");
        } else {
            self.mesh
                .install_spatial_append(&d.spatial)
                .expect("private prepared redo positions");
        }
        drop(mixed_preparation);
        self.state = d.after;
        self.version = next_version;
        registry_change
            .apply(d.after, next_version)
            .expect("private prepared registry redo");
        self.redo_bytes -= d.bytes;
        self.undo_bytes += d.bytes;
        self.undo.push(d);
        Ok(true)
    }
}
// Explicit conservative payload slots, not allocator internals or VM memory.
// Includes coordinates/index slots, adjacency/key slots and preparation copies.
fn sized(count: usize, width: usize) -> Result<usize, String> {
    count
        .checked_mul(width)
        .ok_or_else(|| "Owner payload extent overflow".into())
}
fn add(a: usize, b: usize) -> Result<usize, String> {
    a.checked_add(b)
        .ok_or_else(|| "Owner payload extent overflow".into())
}
fn table_slots(entries: usize, width: usize) -> Result<usize, String> {
    if entries == 0 {
        return Ok(0);
    }
    let slots = entries
        .checked_next_power_of_two()
        .and_then(|n| n.checked_mul(2))
        .ok_or("Table payload extent overflow")?
        .max(4);
    add(
        64,
        sized(
            slots,
            width.checked_add(1).ok_or("Table slot extent overflow")?,
        )?,
    )
}
fn preparation_bound(vertices: usize, edges: usize) -> Result<usize, String> {
    let coordinates = sized(
        vertices,
        2 * std::mem::size_of::<DVec3>() + 2 * std::mem::size_of::<MeshVertex>(),
    )?;
    let index = table_slots(vertices, std::mem::size_of::<([i64; 3], usize)>())?;
    let endpoint_count = edges.checked_mul(2).ok_or("Endpoint extent overflow")?;
    let adjacency = table_slots(endpoint_count, std::mem::size_of::<(usize, Vec<usize>)>())?;
    let touched = table_slots(endpoint_count, std::mem::size_of::<usize>())?;
    let records = sized(
        edges,
        2 * std::mem::size_of::<MeshEdge>() + 16 * std::mem::size_of::<usize>() + 1,
    )?;
    add(
        256,
        add(
            add(coordinates, index)?,
            add(add(adjacency, touched)?, records)?,
        )?,
    )
}
fn staging_bound(
    base_vertices: usize,
    base_edges: usize,
    vertices: usize,
    edges: usize,
) -> Result<usize, String> {
    let nv = base_vertices
        .checked_add(vertices)
        .ok_or("Staging vertex extent overflow")?;
    let ne = base_edges
        .checked_add(edges)
        .ok_or("Staging edge extent overflow")?;
    // Finite f64 rounded registry coordinates fit <=1024 bits. Charge twice
    // that limb payload per component plus table slots and coordinate vectors.
    let limb_bytes = 2 * (f64::MAX_EXP as usize / 8);
    let keys = add(
        table_slots(nv, std::mem::size_of::<([num_bigint::BigInt; 3], usize)>())?,
        sized(nv, 3 * limb_bytes + std::mem::size_of::<DVec3>())?,
    )?;
    let links = add(
        table_slots(
            ne,
            std::mem::size_of::<((usize, usize), (usize, usize, usize))>(),
        )?,
        sized(edges, std::mem::size_of::<(usize, usize)>())?,
    )?;
    add(3 * limb_bytes, add(keys, links)?)
}
fn delta_bound(vertices: usize, edges: usize) -> Result<usize, String> {
    add(
        std::mem::size_of::<Delta>(),
        add(
            sized(vertices, std::mem::size_of::<MeshVertex>())?,
            sized(
                edges,
                std::mem::size_of::<MeshEdge>() + 2 * std::mem::size_of::<usize>(),
            )?,
        )?,
    )
}
fn positions(vertices: &[MeshVertex], extra: usize) -> Result<Vec<DVec3>, String> {
    let mut out = Vec::new();
    out.try_reserve_exact(
        vertices
            .len()
            .checked_add(extra)
            .ok_or("Position extent overflow")?,
    )
    .map_err(|_| "Position preparation refused")?;
    out.extend(vertices.iter().map(|v| v.position));
    Ok(out)
}
// The append adapter owns only fresh unpainted edges, so copying metadata-free
// records needs no hidden String/Value/HashMap clone allocations.
fn copy_append_edges(source: &[MeshEdge]) -> Result<Vec<MeshEdge>, String> {
    let mut out = Vec::new();
    out.try_reserve_exact(source.len())
        .map_err(|_| "Delta edge admission refused")?;
    for e in source {
        if !e.attrs.is_empty()
            || !e.faces.is_empty()
            || e.layer.is_some()
            || e.soft
            || e.curve_id.is_some()
            || e.hidden
        {
            return Err("Non-append edge metadata refused".into());
        }
        out.push(MeshEdge::new(e.v0, e.v1));
    }
    Ok(out)
}
fn copy_vertices(source: &[MeshVertex]) -> Result<Vec<MeshVertex>, String> {
    let mut out = Vec::new();
    out.try_reserve_exact(source.len())
        .map_err(|_| "Delta vertex admission refused")?;
    for v in source {
        let mut edges = Vec::new();
        edges
            .try_reserve_exact(v.edges.len())
            .map_err(|_| "Delta adjacency admission refused")?;
        edges.extend_from_slice(&v.edges);
        out.push(MeshVertex {
            uid: v.uid,
            position: v.position,
            edges,
        });
    }
    Ok(out)
}
fn delta_bytes(vertices: &Vec<MeshVertex>, edges: &Vec<MeshEdge>) -> Result<usize, String> {
    let mut bytes = std::mem::size_of::<Delta>();
    for (count, width) in [
        (vertices.capacity(), std::mem::size_of::<MeshVertex>()),
        (edges.capacity(), std::mem::size_of::<MeshEdge>()),
    ] {
        bytes = bytes
            .checked_add(
                count
                    .checked_mul(width)
                    .ok_or("Delta byte extent overflow")?,
            )
            .ok_or("Delta byte extent overflow")?;
    }
    for v in vertices {
        bytes = bytes
            .checked_add(
                v.edges
                    .capacity()
                    .checked_mul(std::mem::size_of::<usize>())
                    .ok_or("Delta adjacency byte extent overflow")?,
            )
            .ok_or("Delta byte extent overflow")?;
    }
    Ok(bytes)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn spatial_delta_native_collisions_restore_exact_predecessor_and_reject_stale() {
        let mut m = Mesh::new();
        m.vertices.push(MeshVertex::new(DVec3::ZERO));
        m.vertices.push(MeshVertex::new(DVec3::ZERO));
        m.rebuild_spatial_index();
        let points = [DVec3::ZERO, DVec3::X, DVec3::ZERO];
        let patch = m.prepare_spatial_append(&points).unwrap();
        for p in points {
            m.vertices.push(MeshVertex::new(p));
        }
        m.install_spatial_append(&patch).unwrap();
        assert_eq!(m.add_vertex(DVec3::ZERO), 4);
        m.restore_spatial_append(&patch).unwrap();
        m.vertices.truncate(2);
        assert_eq!(m.add_vertex(DVec3::ZERO), 1);
        assert!(m.restore_spatial_append(&patch).is_err());
        m.reserve_spatial_append(&patch).unwrap();
        for p in points {
            m.vertices.push(MeshVertex::new(p));
        }
        m.install_spatial_append(&patch).unwrap();
        assert_eq!(m.add_vertex(DVec3::ZERO), 4);
        let before = m.clone();
        m.vertices[4].position = DVec3::Y;
        assert!(m.restore_spatial_append(&patch).is_err());
        m.vertices[4].position = DVec3::ZERO;
        assert_eq!(m, before);
        assert!(m.prepare_spatial_append(&[DVec3::NAN]).is_err());
        assert_eq!(m, before);
    }
    #[test]
    fn persistent_registry_stale_before_lookup_and_failed_prepare_preserves_view() {
        let mut o = JournalOwner::new(Mesh::new()).unwrap();
        for action in 0..4 {
            let mut held = o.try_begin().unwrap();
            match action {
                0 => {
                    let mut t = o.try_begin().unwrap();
                    t.add_edge(DVec3::ZERO, DVec3::X).unwrap();
                    o.commit(t).unwrap();
                }
                1 => {
                    o.undo().unwrap();
                }
                2 => {
                    o.redo().unwrap();
                }
                _ => {
                    o.replace(Mesh::new()).unwrap();
                }
            }
            let calls = held.registry_lookup_calls();
            assert!(held
                .add_edge(DVec3::Y, DVec3::Z)
                .unwrap_err()
                .contains("Stale source registry"));
            assert_eq!(held.registry_lookup_calls(), calls);
        }
        let mut earlier = o.try_begin().unwrap();
        let mut candidate = o.try_begin().unwrap();
        candidate.add_edge(DVec3::ZERO, DVec3::X).unwrap();
        o.set_index_entry_limit(Some(0));
        assert!(o.commit(candidate).is_err());
        assert_eq!(o.history_counts(), (0, 0));
        earlier.add_edge(DVec3::Y, DVec3::Z).unwrap();
        o.set_index_entry_limit(None);
        assert!(o.commit(earlier).unwrap());
    }
    #[test]
    fn persistent_registry_duplicate_vertex_last_wins_after_undo() {
        let mut m = Mesh::new();
        m.vertices.push(MeshVertex::new(DVec3::ZERO));
        m.vertices.push(MeshVertex::new(DVec3::ZERO));
        let mut o = JournalOwner::new(m).unwrap();
        for endpoint in [DVec3::X, DVec3::Y] {
            let mut t = o.try_begin().unwrap();
            t.add_edge(DVec3::ZERO, endpoint).unwrap();
            o.commit(t).unwrap();
            assert_eq!(o.mesh().edges[0].v0, 1);
            o.undo().unwrap();
            assert_eq!(o.mesh().vertices.len(), 2);
        }
        assert_eq!(o.registry_metrics().cold_vertex_rows, 2);
    }
    #[test]
    fn persistent_registry_lifetime_stale_views_and_duplicate_base_edge_ids() {
        let mut mesh = Mesh::new();
        let a = mesh.add_vertex(DVec3::ZERO);
        let b = mesh.add_vertex(DVec3::X);
        mesh.add_edge(a, b);
        let mut duplicate = MeshEdge::new(a, b);
        duplicate.layer = Some("opaque".into());
        mesh.vertices[a].edges.push(1);
        mesh.vertices[b].edges.push(1);
        mesh.edges.push(duplicate);
        let mut o = JournalOwner::new(mesh).unwrap();
        let budget = o.budget();
        let mut t = o.try_begin().unwrap();
        t.add_edge(DVec3::Y, DVec3::Z).unwrap();
        o.commit(t).unwrap();
        assert_eq!(o.mesh().edges[2].v0, 2);
        assert_eq!(o.mesh().edges[2].v1, 3);
        o.undo().unwrap();
        o.redo().unwrap();
        let mut duplicate = o.try_begin().unwrap();
        duplicate.add_edge(DVec3::ZERO, DVec3::X).unwrap();
        assert!(!o.commit(duplicate).unwrap());
        let held = o.try_begin().unwrap();
        let mut stale = o.try_begin().unwrap();
        let retained = budget.snapshot().registry;
        assert!(retained > 0);
        o.replace(Mesh::new()).unwrap();
        assert_eq!(budget.snapshot().registry, retained);
        assert!(stale
            .add_edge(DVec3::ZERO, DVec3::X)
            .unwrap_err()
            .contains("Stale source registry"));
        assert!(o.commit(stale).is_err());
        drop(held);
        assert_eq!(budget.snapshot().registry, 0);
    }
    #[test]
    fn persistent_registry_begin_no_base_scan_and_overlay_lookup_order() {
        let mut mesh = Mesh::new();
        mesh.add_vertex(DVec3::ZERO);
        let mut o = JournalOwner::new(mesh).unwrap();
        let identity = o.registry_identity();
        let scans = o.registry_metrics().cold_vertex_rows;
        for _ in 0..100 {
            let t = o.try_begin().unwrap();
            drop(t);
        }
        assert_eq!(o.registry_metrics().cold_vertex_rows, scans);
        assert_eq!(o.registry_identity(), identity);
        let mut t = o.try_begin().unwrap();
        t.add_edge(DVec3::ZERO, DVec3::X).unwrap();
        t.add_edge(DVec3::X, DVec3::Y).unwrap();
        o.commit(t).unwrap();
        assert_eq!(o.mesh().vertices.len(), 3);
        assert_eq!(o.mesh().edges[1].v0, 1);
        assert_eq!(o.mesh().edges[1].v1, 2);
        assert_eq!(o.registry_metrics().publication_rows, 4);
        o.undo().unwrap();
        assert_eq!(o.mesh().vertices.len(), 1);
        o.redo().unwrap();
        let mut repeat = o.try_begin().unwrap();
        repeat.add_edge(DVec3::X, DVec3::Y).unwrap();
        assert!(!o.commit(repeat).unwrap());
        assert_eq!(o.registry_identity(), identity);
    }

    #[test]
    fn shared_budget_two_owners_and_existing_redo_survive_refused_commit() {
        let budget = OwnerBudget::new(None);
        let mut a = JournalOwner::new_with_budget(Mesh::new(), budget.clone()).unwrap();
        let mut b = JournalOwner::new_with_budget(Mesh::new(), budget.clone()).unwrap();
        for o in [&mut a, &mut b] {
            let mut t = o.try_begin().unwrap();
            t.add_edge(DVec3::ZERO, DVec3::X).unwrap();
            o.commit(t).unwrap();
        }
        assert_eq!(
            budget.snapshot().history,
            a.history_payload_bytes().0 + b.history_payload_bytes().0
        );
        a.undo().unwrap();
        let old_a = a.mesh().clone();
        let old_b = b.mesh().clone();
        let history = budget.snapshot().history;
        assert_eq!(
            history,
            a.history_payload_bytes().1 + b.history_payload_bytes().0
        );
        let mut candidate = a.try_begin().unwrap();
        candidate.add_edge(DVec3::Y, DVec3::Z).unwrap();
        let before = budget.snapshot();
        assert!(before.staging > 0);
        budget.set_limit(Some(before.total()));
        assert!(a
            .commit(candidate)
            .unwrap_err()
            .contains("Shared owner payload admission"));
        assert_eq!(a.mesh(), &old_a);
        assert_eq!(b.mesh(), &old_b);
        assert_eq!(a.history_counts(), (0, 1));
        assert_eq!(b.history_counts(), (1, 0));
        assert_eq!(
            (
                budget.snapshot().history,
                budget.snapshot().staging,
                budget.snapshot().preparation
            ),
            (history, 0, 0)
        );
        budget.set_limit(None);
        a.redo().unwrap();
        assert_eq!(budget.snapshot().history, history);
        drop(a);
        assert_eq!(budget.snapshot().history, b.history_payload_bytes().0);
        drop(b);
        assert_eq!(budget.snapshot().total(), 0);
    }

    #[test]
    fn shared_budget_constructor_and_preparation_refusals_preserve_owner() {
        let budget = OwnerBudget::new(Some(0));
        assert!(JournalOwner::new_with_budget(Mesh::new(), budget.clone()).is_err());
        assert_eq!(budget.snapshot().total(), 0);
        budget.set_limit(None);
        let mut o = JournalOwner::new_with_budget(Mesh::new(), budget.clone()).unwrap();
        let mut t = o.try_begin().unwrap();
        t.add_edge(DVec3::ZERO, DVec3::X).unwrap();
        o.commit(t).unwrap();
        let saved = o.mesh().clone();
        let history = o.history_counts();
        let before = budget.snapshot();
        assert!(before.history > 0);
        assert_eq!((before.staging, before.preparation), (0, 0));
        let valid = o.try_begin().unwrap();
        budget.set_limit(Some(0));
        assert!(o.undo().is_err());
        assert_eq!(o.mesh(), &saved);
        assert_eq!(o.history_counts(), history);
        assert!(o.replace(Mesh::new()).is_err());
        assert_eq!(o.mesh(), &saved);
        assert_eq!(o.history_counts(), history);
        assert!(!o.commit(valid).unwrap());
        assert_eq!(budget.snapshot().history, before.history);
        assert_eq!(budget.snapshot().staging, 0);
        budget.set_limit(None);
        o.undo().unwrap();
        let undone = o.mesh().clone();
        let retained = budget.snapshot().history;
        budget.set_limit(Some(retained));
        assert!(o.redo().is_err());
        assert_eq!(o.mesh(), &undone);
        assert_eq!(o.history_counts(), (0, 1));
        assert_eq!(
            (budget.snapshot().staging, budget.snapshot().preparation),
            (0, 0)
        );
        budget.set_limit(None);
        o.redo().unwrap();
        assert_eq!(o.mesh(), &saved);
        o.replace(Mesh::new()).unwrap();
        assert_eq!(budget.snapshot().total(), 0);
    }
    #[test]
    fn shared_budget_staging_and_commit_refusals_release_candidate_only() {
        let mut o = JournalOwner::new(Mesh::new()).unwrap();
        let budget = o.budget();
        let t = o.try_begin().unwrap();
        let first = budget.snapshot().staging;
        assert!(first > 0);
        let second = o.try_begin().unwrap();
        assert_eq!(budget.snapshot().staging, first * 2);
        drop(second);
        assert_eq!(budget.snapshot().staging, first);
        drop(t);
        assert_eq!(budget.snapshot().total(), 0);
        let mut t = o.try_begin().unwrap();
        budget.set_limit(Some(budget.snapshot().total()));
        assert!(t.add_edge(DVec3::ZERO, DVec3::X).is_err());
        assert!(o.commit(t).is_err());
        assert!(o.mesh().vertices.is_empty());
        assert_eq!(budget.snapshot().total(), 0);
        budget.set_limit(None);
        let mut t = o.try_begin().unwrap();
        t.add_edge(DVec3::ZERO, DVec3::X).unwrap();
        budget.set_limit(Some(budget.snapshot().total()));
        assert!(o.commit(t).is_err());
        assert!(o.mesh().vertices.is_empty());
        assert_eq!(o.history_counts(), (0, 0));
        assert_eq!(budget.snapshot().total(), 0);
        budget.set_limit(None);
        let mut t = o.try_begin().unwrap();
        t.add_edge(DVec3::ZERO, DVec3::X).unwrap();
        assert!(o.commit(t).unwrap());
        assert_eq!(budget.snapshot().history, o.history_payload_bytes().0);
        drop(o);
        assert_eq!(budget.snapshot().total(), 0);
    }

    #[test]
    fn caught_staging_refusal_cannot_publish_partial_vertices() {
        let mut o = JournalOwner::new(Mesh::new()).unwrap();
        o.set_append_limits(Some(1), None);
        let mut t = o.try_begin().unwrap();
        assert!(t.add_edge(DVec3::ZERO, DVec3::X).is_err());
        assert!(t
            .add_edge(DVec3::Y, DVec3::Z)
            .unwrap_err()
            .contains("previously refused"));
        assert!(o
            .commit(t)
            .unwrap_err()
            .contains("staging admission failed"));
        assert!(o.mesh().vertices.is_empty());
        assert_eq!(o.history_counts(), (0, 0));
        o.set_append_limits(None, Some(0));
        let mut t = o.try_begin().unwrap();
        assert!(t.add_edge(DVec3::ZERO, DVec3::X).is_err());
        assert!(o.commit(t).is_err());
        assert!(o.mesh().vertices.is_empty());
        o.set_append_limits(None, None);
        let mut t = o.try_begin().unwrap();
        t.add_edge(DVec3::ZERO, DVec3::X).unwrap();
        o.commit(t).unwrap();
        assert_eq!(o.mesh().edges.len(), 1);
        o.set_append_limits(Some(0), Some(0));
        let mut duplicate = o.try_begin().unwrap();
        duplicate.add_edge(DVec3::ZERO, DVec3::X).unwrap();
        assert!(!o.commit(duplicate).unwrap());
        assert_eq!(o.history_counts(), (1, 0));
    }

    #[test]
    fn history_admission_preserves_redo_and_retry_without_payload_growth() {
        let mut o = JournalOwner::new(Mesh::new()).unwrap();
        let mut t = o.begin();
        t.add_edge(DVec3::ZERO, DVec3::X).unwrap();
        o.commit(t).unwrap();
        let first = o.history_payload_bytes();
        assert!(first.0 > 0);
        assert_eq!(first.1, 0);
        let saved = o.mesh().clone();
        o.undo().unwrap();
        assert_eq!(o.history_payload_bytes(), (0, first.0));
        o.set_history_byte_limit(Some(0));
        let mut branch = o.begin();
        branch.add_edge(DVec3::Y, DVec3::Z).unwrap();
        assert!(o.commit(branch).unwrap_err().contains("payload admission"));
        assert!(o.mesh().vertices.is_empty());
        assert_eq!(o.history_counts(), (0, 1));
        assert_eq!(o.history_payload_bytes(), (0, first.0));
        // Existing history moves between stacks without requiring a new admission.
        o.redo().unwrap();
        assert_eq!(o.mesh(), &saved);
        assert_eq!(o.history_payload_bytes(), first);
        o.set_history_byte_limit(Some(first.0));
        let mut t = o.begin();
        t.add_edge(DVec3::Y, DVec3::Z).unwrap();
        assert!(o.commit(t).is_err());
        assert_eq!(o.mesh(), &saved);
        assert_eq!(o.history_payload_bytes(), first);
        o.set_history_byte_limit(None);
        let mut t = o.begin();
        t.add_edge(DVec3::Y, DVec3::Z).unwrap();
        o.commit(t).unwrap();
        assert!(o.history_payload_bytes().0 > first.0);
        o.replace(Mesh::new()).unwrap();
        assert_eq!(o.history_payload_bytes(), (0, 0));
    }

    #[test]
    fn quota_failure_preserves_mesh_history_and_retry() {
        let mut o = JournalOwner::new(Mesh::new()).unwrap();
        o.set_index_entry_limit(Some(1));
        let mut t = o.begin();
        t.add_edge(DVec3::ZERO, DVec3::X).unwrap();
        assert!(o.commit(t).is_err());
        assert!(o.mesh().vertices.is_empty());
        assert_eq!(o.history_counts(), (0, 0));
        o.set_index_entry_limit(None);
        let mut t = o.begin();
        t.add_edge(DVec3::ZERO, DVec3::X).unwrap();
        assert!(o.commit(t).unwrap());
        let old = o.mesh().clone();
        o.undo().unwrap();
        o.set_index_entry_limit(Some(1));
        assert!(o.redo().is_err());
        assert!(o.mesh().vertices.is_empty());
        assert_eq!(o.history_counts(), (0, 1));
        o.set_index_entry_limit(None);
        o.redo().unwrap();
        assert_eq!(o.mesh(), &old);
    }
    #[test]
    fn failed_undo_and_replacement_keep_published_state() {
        let mut m = Mesh::new();
        for p in [DVec3::ZERO, DVec3::X, DVec3::Y] {
            m.add_vertex(p);
        }
        let mut o = JournalOwner::new(m).unwrap();
        let mut t = o.begin();
        t.add_edge(DVec3::Y, DVec3::Z).unwrap();
        o.commit(t).unwrap();
        let before = o.mesh().clone();
        o.set_index_entry_limit(Some(1));
        assert!(o.undo().is_err());
        assert_eq!(o.mesh(), &before);
        assert_eq!(o.history_counts(), (1, 0));
        assert!(o.replace(before.clone()).is_err());
        assert_eq!(o.mesh(), &before);
        assert_eq!(o.history_counts(), (1, 0));
    }
    #[test]
    fn prepared_index_refuses_stale_positions_without_lookup_change() {
        let mut mesh = Mesh::new();
        mesh.add_vertex(DVec3::ZERO);
        let index = Mesh::prepare_spatial_index(&[DVec3::X]).unwrap();
        assert!(mesh.install_prepared_spatial_index(index).is_err());
        assert_eq!(mesh.add_vertex(DVec3::ZERO), 0);
        assert_eq!(mesh.vertices.len(), 1);
    }
    #[test]
    fn stale_transaction_after_replace_undo_redo_refuses() {
        let mut o = JournalOwner::new(Mesh::new()).unwrap();
        let mut stale = o.begin();
        stale.add_edge(DVec3::ZERO, DVec3::X).unwrap();
        o.replace(Mesh::new()).unwrap();
        assert!(o.commit(stale).is_err());
        let mut t = o.begin();
        t.add_edge(DVec3::ZERO, DVec3::X).unwrap();
        o.commit(t).unwrap();
        let stale = o.begin();
        o.undo().unwrap();
        assert!(o.commit(stale).is_err());
        let stale = o.begin();
        o.redo().unwrap();
        assert!(o.commit(stale).is_err());
        assert_eq!(o.history_counts(), (1, 0));
    }
}

/// Existing-field publication prerequisite. A raw Face.loop edit does not heal
/// radial records and a raw QVector edit does not re-key the source registry.
/// Command history is owned by the future VM controller; this ticket does not
/// impersonate append Undo and makes prior native append tokens stale.
pub struct PreparedFields<'a> {
    owner: &'a mut JournalOwner,
    positions: Vec<DVec3>,
    loops: Vec<(Vec<usize>, Vec<Vec<usize>>)>,
    spatial: PreparedSpatialIndex,
    registry: crate::owner_registry::PositionChange,
    after: Uuid,
    next: u64,
    _charge: Lease,
}
impl JournalOwner {
    pub fn prepare_fields(
        &mut self,
        positions: Vec<DVec3>,
        loops: Vec<(Vec<usize>, Vec<Vec<usize>>)>,
    ) -> Result<PreparedFields<'_>, String> {
        if positions.len() != self.mesh.vertices.len() || loops.len() != self.mesh.faces.len() {
            return Err("Raw field projection requires unchanged membership".into());
        }
        self.prepare_fields_as(positions, loops, Uuid::new_v4())
    }
    pub(crate) fn prepare_fields_as(
        &mut self,
        positions: Vec<DVec3>,
        loops: Vec<(Vec<usize>, Vec<Vec<usize>>)>,
        after: Uuid,
    ) -> Result<PreparedFields<'_>, String> {
        if positions.len() != self.mesh.vertices.len() || loops.len() != self.mesh.faces.len() {
            return Err("Raw field projection requires unchanged membership".into());
        }
        let next = self
            .version
            .checked_add(1)
            .ok_or("Owner revision overflow")?;
        for (outer, holes) in &loops {
            for &id in outer.iter().chain(holes.iter().flatten()) {
                if id >= positions.len() {
                    return Err("Raw field projection vertex identity outside membership".into());
                }
            }
        }
        let charge = self
            .budget
            .reserve(Kind::Preparation, preparation_bound(positions.len(), 0)?)?;
        let spatial = self.index(&positions)?;
        let registry = self.registry.prepare_positions(&positions)?;
        Ok(PreparedFields {
            owner: self,
            positions,
            loops,
            spatial,
            registry,
            after,
            next,
            _charge: charge,
        })
    }
}
impl PreparedFields<'_> {
    pub fn apply(self) {
        let Self {
            owner,
            positions,
            loops,
            spatial,
            registry,
            after,
            next,
            _charge,
        } = self;
        for (v, p) in owner.mesh.vertices.iter_mut().zip(positions) {
            v.position = p;
        }
        for (f, (outer, holes)) in owner.mesh.faces.iter_mut().zip(loops) {
            f.vertices = outer;
            f.holes = holes;
        }
        owner
            .mesh
            .install_prepared_spatial_index(spatial)
            .expect("Exclusive prepared raw field positions");
        registry.apply(after, next);
        owner.state = after;
        owner.version = next;
    }
}

#[cfg(test)]
mod mixed_append_tests {
    use super::*;
    #[test]
    fn held_mixed_index_preparation_remains_charged_until_discard() {
        let mut mesh = Mesh::new();
        mesh.add_vertex(DVec3::ZERO);
        let budget = OwnerBudget::new(None);
        let mut owner = JournalOwner::new_with_budget(mesh, budget.clone()).unwrap();
        let baseline = budget.snapshot();
        let mut tx = owner.try_begin().unwrap();
        tx.add_edge(DVec3::ZERO, DVec3::Y).unwrap();
        let ticket = owner.prepare(tx).unwrap().unwrap();
        let append = budget.snapshot();
        let ticket = ticket.with_existing_fields(vec![DVec3::X], vec![]).unwrap();
        let mixed = budget.snapshot();
        assert!(mixed.preparation >= append.preparation + preparation_bound(2, 0).unwrap());
        budget.set_limit(Some(mixed.total()));
        assert!(budget.reserve(Kind::Preparation, 1).is_err());
        drop(ticket);
        budget.set_limit(None);
        assert_eq!(budget.snapshot().total(), baseline.total());
        assert_eq!(owner.mesh.vertices[0].position, DVec3::ZERO);
        assert!(owner.undo.is_empty());
    }
    #[test]
    fn raw_position_and_append_share_one_reversible_owner_command() {
        let mut mesh = Mesh::new();
        mesh.add_vertex(DVec3::ZERO);
        let mut owner = JournalOwner::new(mesh).unwrap();
        let before = owner.state;
        let stale = owner.try_begin().unwrap();
        let mut tx = owner.try_begin().unwrap();
        tx.add_edge(DVec3::ZERO, DVec3::Y).unwrap();
        owner
            .prepare(tx)
            .unwrap()
            .unwrap()
            .with_existing_fields(vec![DVec3::X], vec![])
            .unwrap()
            .apply();
        assert!(owner.prepare(stale).is_err());
        assert_eq!(owner.registry.metrics().position_rows, 1);
        assert_eq!(owner.mesh.vertices[0].position, DVec3::X);
        assert_eq!(owner.mesh.vertices.len(), 2);
        assert_eq!(owner.mesh.edges.len(), 1);
        assert_eq!(
            owner
                .registry
                .view()
                .vertex(&crate::journal::SourceKey::new(DVec3::ZERO)),
            Some(0)
        );
        assert!(owner.undo().unwrap());
        assert_eq!(owner.state, before);
        assert_eq!(owner.mesh.vertices.len(), 1);
        assert_eq!(owner.mesh.vertices[0].position, DVec3::ZERO);
        assert!(owner.redo().unwrap());
        assert_eq!(owner.mesh.vertices[0].position, DVec3::X);
        assert_eq!(owner.mesh.vertices.len(), 2);
        let budget = owner.budget.clone();
        let charge = budget.snapshot().total();
        budget.set_limit(Some(charge));
        assert!(owner.undo().is_err());
        assert_eq!(owner.mesh.vertices[0].position, DVec3::X);
        assert_eq!(owner.mesh.vertices.len(), 2);
        assert_eq!(budget.snapshot().total(), charge);
        budget.set_limit(None);
        assert!(owner.undo().unwrap());
        assert!(owner.redo().unwrap());
    }
}

/// Exclusive pure membership permutation, preserving native record identities.
/// Full insertion/deletion/replacement graph ownership is a separate obligation.
pub struct PreparedMembership<'a> {
    edge_swaps: Vec<(usize,usize)>,
    edge_map: Option<Vec<usize>>,
    owner: &'a mut JournalOwner,
    vertex_swaps: Vec<(usize, usize)>,
    face_swaps: Vec<(usize, usize)>,
    vertex_map: Vec<usize>,
    face_map: Vec<usize>,
    positions: Vec<DVec3>,
    loops: Vec<(Vec<usize>, Vec<Vec<usize>>)>,
    spatial: PreparedSpatialIndex,
    registry: crate::owner_registry::PermutationChange,
    after: Uuid,
    next: u64,
    _preparation: Lease,
}
fn prepare_order(
    order: &[usize],
    extent: usize,
) -> Result<(Vec<usize>, Vec<(usize, usize)>), String> {
    if order.len() != extent {
        return Err("Membership permutation extent mismatch".into());
    }
    let mut inverse = Vec::new();
    inverse
        .try_reserve_exact(extent)
        .map_err(|_| "Membership inverse refused")?;
    inverse.resize(extent, usize::MAX);
    for (new, &old) in order.iter().enumerate() {
        if old >= extent || inverse[old] != usize::MAX {
            return Err("Membership requires identity bijection".into());
        }
        inverse[old] = new;
    }
    let mut current = Vec::new();
    current
        .try_reserve_exact(extent)
        .map_err(|_| "Membership current order refused")?;
    current.extend(0..extent);
    let mut location = Vec::new();
    location
        .try_reserve_exact(extent)
        .map_err(|_| "Membership object location refused")?;
    location.extend(0..extent);
    let mut swaps = Vec::new();
    swaps
        .try_reserve_exact(extent)
        .map_err(|_| "Membership swaps refused")?;
    for (id, &old) in order.iter().enumerate() {
        let at = location[old];
        if at != id {
            let other = current[id];
            current.swap(id, at);
            location[old] = id;
            location[other] = at;
            swaps.push((id, at));
        }
    }
    Ok((inverse, swaps))
}
impl JournalOwner {
    pub fn prepare_membership_as(
        &mut self,
        vertices: &[usize],
        faces: &[usize],
        positions: Vec<DVec3>,
        loops: Vec<(Vec<usize>, Vec<Vec<usize>>)>,
        after: Uuid,
    ) -> Result<PreparedMembership<'_>, String> {
        self.prepare_graph_order_inner(vertices,None,faces,positions,loops,after)
    }
    pub fn prepare_graph_order_as(&mut self,vertices:&[usize],edges:&[usize],faces:&[usize],positions:Vec<DVec3>,loops:Vec<(Vec<usize>,Vec<Vec<usize>>)>,after:Uuid)->Result<PreparedMembership<'_>,String>{
        self.prepare_graph_order_inner(vertices,Some(edges),faces,positions,loops,after)
    }
    fn prepare_graph_order_inner(&mut self,vertices:&[usize],edges:Option<&[usize]>,faces:&[usize],positions:Vec<DVec3>,loops:Vec<(Vec<usize>,Vec<Vec<usize>>)>,after:Uuid)->Result<PreparedMembership<'_>,String>{
        if positions.len() != self.mesh.vertices.len() || loops.len() != self.mesh.faces.len() {
            return Err("Membership field extent mismatch".into());
        }
        let next = self
            .version
            .checked_add(1)
            .ok_or("Membership owner revision overflow")?;
        let bytes = preparation_bound(positions.len(), self.mesh.edges.len())?
            .checked_add(sized(add(positions.len(), loops.len())?, 128)?)
            .ok_or("Membership preparation overflow")?;
        let bytes=bytes.checked_add(if edges.is_some(){self.mesh.edges.len().checked_mul(5*std::mem::size_of::<usize>()).ok_or("Edge membership preparation overflow")?}else{0}).ok_or("Graph membership preparation overflow")?;
        let preparation = self.budget.reserve(Kind::Preparation, bytes)?;
        let (edge_map,edge_swaps)=match edges{Some(order)=>{let(map,swaps)=prepare_order(order,self.mesh.edges.len())?;(Some(map),swaps)},None=>(None,Vec::new())};
        let (vertex_map, vertex_swaps) = prepare_order(vertices, self.mesh.vertices.len())?;
        let (face_map, face_swaps) = prepare_order(faces, self.mesh.faces.len())?;
        for edge in &self.mesh.edges {
            if edge.v0 >= vertex_map.len()
                || edge.v1 >= vertex_map.len()
                || edge.faces.iter().any(|&id| id >= face_map.len())
            {
                return Err("Membership native reference outside graph".into());
            }
        }
        for (outer, holes) in &loops {
            if outer
                .iter()
                .chain(holes.iter().flatten())
                .any(|&id| id >= positions.len())
            {
                return Err("Membership loop reference outside graph".into());
            }
        }
        if let Some(map)=&edge_map {if self.mesh.vertices.iter().any(|v|v.edges.iter().any(|&id|id>=map.len())){return Err("Membership native incidence outside graph".into());}}
        let spatial = self.index(&positions)?;
        let registry = self.registry.prepare_graph_permutation(&vertex_map,edge_map.as_deref(), &positions)?;
        Ok(PreparedMembership {
            edge_swaps,edge_map,
            owner: self,
            vertex_swaps,
            face_swaps,
            vertex_map,
            face_map,
            positions,
            loops,
            spatial,
            registry,
            after,
            next,
            _preparation: preparation,
        })
    }
}
impl PreparedMembership<'_> {
    pub fn apply(self) {
        let Self {
            edge_swaps,edge_map,
            owner,
            vertex_swaps,
            face_swaps,
            vertex_map,
            face_map,
            positions,
            loops,
            spatial,
            registry,
            after,
            next,
            _preparation,
        } = self;
        for (a,b) in edge_swaps {owner.mesh.edges.swap(a,b);}
        for (a, b) in vertex_swaps {
            owner.mesh.vertices.swap(a, b);
        }
        if let Some(map)=edge_map {for v in &mut owner.mesh.vertices{for id in &mut v.edges{*id=map[*id];}}}
        for (a, b) in face_swaps {
            owner.mesh.faces.swap(a, b);
        }
        for (v, p) in owner.mesh.vertices.iter_mut().zip(positions) {
            v.position = p;
        }
        for (f, (outer, holes)) in owner.mesh.faces.iter_mut().zip(loops) {
            f.vertices = outer;
            f.holes = holes;
        }
        for edge in &mut owner.mesh.edges {
            edge.v0 = vertex_map[edge.v0];
            edge.v1 = vertex_map[edge.v1];
            for id in &mut edge.faces {
                *id = face_map[*id];
            }
        }
        owner
            .mesh
            .install_prepared_spatial_index(spatial)
            .expect("Exclusive prepared membership positions");
        registry.apply(after, next);
        owner.state = after;
        owner.version = next;
    }
}
#[cfg(test)]
mod membership_tests {
    use super::*;
    #[test]
    fn permutation_metadata_and_incidence_publish_only_after_admission() {
        let mut mesh = Mesh::new();
        for p in [DVec3::ZERO, DVec3::X, DVec3::Y, DVec3::Z] {
            mesh.add_vertex(p);
        }
        mesh.add_face(vec![0, 1, 2], vec![]).unwrap();
        mesh.add_face(vec![1, 2, 3], vec![]).unwrap();
        mesh.faces[0]
            .attrs
            .insert("vendor".into(), serde_json::json!([1, 2, 3]));
        mesh.faces[1]
            .attrs
            .insert("vendor".into(), serde_json::json!([4, 5, 6]));
        let first = mesh.faces[0].attrs["vendor"].as_array().unwrap().as_ptr();
        let second = mesh.faces[1].attrs["vendor"].as_array().unwrap().as_ptr();
        let budget = OwnerBudget::new(None);
        let mut owner = JournalOwner::new_with_budget(mesh, budget.clone()).unwrap();
        let before = budget.snapshot().total();
        let ticket = owner
            .prepare_membership_as(
                &[3, 2, 1, 0],
                &[1, 0],
                vec![DVec3::Z, DVec3::Y, DVec3::X, DVec3::ZERO],
                vec![(vec![2, 1, 0], vec![]), (vec![3, 2, 1], vec![])],
                Uuid::new_v4(),
            )
            .unwrap();
        let held = budget.snapshot().total();
        assert!(held > before);
        budget.set_limit(Some(held));
        assert!(budget.reserve(Kind::Preparation, 1).is_err());
        drop(ticket);
        budget.set_limit(None);
        assert_eq!(budget.snapshot().total(), before);
        assert_eq!(
            owner.mesh.faces[0].attrs["vendor"]
                .as_array()
                .unwrap()
                .as_ptr(),
            first
        );
        owner
            .prepare_membership_as(
                &[3, 2, 1, 0],
                &[1, 0],
                vec![DVec3::Z, DVec3::Y, DVec3::X, DVec3::ZERO],
                vec![(vec![2, 1, 0], vec![]), (vec![3, 2, 1], vec![])],
                Uuid::new_v4(),
            )
            .unwrap()
            .apply();
        assert_eq!(
            owner.mesh.faces[0].attrs["vendor"]
                .as_array()
                .unwrap()
                .as_ptr(),
            second
        );
        assert_eq!(
            owner.mesh.faces[1].attrs["vendor"]
                .as_array()
                .unwrap()
                .as_ptr(),
            first
        );
        assert_eq!((owner.mesh.edges[0].v0, owner.mesh.edges[0].v1), (3, 2));
        assert_eq!(owner.mesh.edges[0].faces, vec![1]);
        assert_eq!(owner.mesh.edges[1].faces, vec![1, 0]);
        assert_eq!(budget.snapshot().total(), before);
    }

    #[test]
    fn pure_order_moves_records_and_registry_without_coordinate_matching() {
        let mut m = Mesh::new();
        m.add_vertex(DVec3::ZERO);
        m.add_vertex(DVec3::X);
        let mut owner = JournalOwner::new(m).unwrap();
        let uids = owner
            .mesh
            .vertices
            .iter()
            .map(|v| v.uid)
            .collect::<Vec<_>>();
        let state = owner.state;
        let stale = owner.try_begin().unwrap();
        owner
            .prepare_membership_as(
                &[1, 0],
                &[],
                vec![DVec3::X, DVec3::ZERO],
                vec![],
                Uuid::new_v4(),
            )
            .unwrap()
            .apply();
        assert_eq!(owner.mesh.vertices[0].uid, uids[1]);
        assert_eq!(
            owner
                .registry
                .view()
                .vertex(&crate::journal::SourceKey::new(DVec3::ZERO)),
            Some(1)
        );
        assert!(owner.prepare(stale).is_err());
        owner
            .prepare_membership_as(&[1, 0], &[], vec![DVec3::ZERO, DVec3::X], vec![], state)
            .unwrap()
            .apply();
        assert_eq!(owner.mesh.vertices[0].uid, uids[0]);
        assert_eq!(
            owner
                .registry
                .view()
                .vertex(&crate::journal::SourceKey::new(DVec3::ZERO)),
            Some(0)
        );
    }
}

#[cfg(test)]mod graph_order_tests{
 use super::*;
 #[test]fn graph_order_preserves_registry_identity_and_refuses_before_mutation(){
  let mut mesh=Mesh::new();for p in [DVec3::ZERO,DVec3::X,DVec3::Y,DVec3::Z]{mesh.add_vertex(p);}
  mesh.add_face(vec![0,1,2],vec![]).unwrap();mesh.add_face(vec![1,0,3],vec![]).unwrap();
  mesh.edges[0].attrs.insert("opaque".into(),serde_json::json!(["vendor",7]));
  let payload=mesh.edges[0].attrs["opaque"].as_array().unwrap().as_ptr();
  let budget=OwnerBudget::new(None);let mut owner=JournalOwner::new_with_budget(mesh,budget.clone()).unwrap();
  let before=owner.mesh().clone();let before_version=owner.version;let history=owner.history_counts();let charge=budget.snapshot().total();
  let view=owner.registry.view();let stale=owner.try_begin().unwrap();
  let positions=vec![DVec3::Z,DVec3::Y,DVec3::X,DVec3::ZERO];let loops=vec![(vec![2,3,0],vec![]),(vec![3,2,1],vec![])];
  let ticket=owner.prepare_graph_order_as(&[3,2,1,0],&[4,3,2,1,0],&[1,0],positions.clone(),loops.clone(),Uuid::new_v4()).unwrap();drop(ticket);
  assert_eq!(owner.mesh(),&before);assert_eq!(owner.version,before_version);assert_eq!(owner.history_counts(),history);assert!(view.check().is_ok());
  let with_stale=budget.snapshot().total();budget.set_limit(Some(with_stale));
  assert!(owner.prepare_graph_order_as(&[3,2,1,0],&[4,3,2,1,0],&[1,0],positions.clone(),loops.clone(),Uuid::new_v4()).is_err());budget.set_limit(None);
  for invalid in [&[4,3,2,1,1][..],&[5,3,2,1,0][..],&[4,3,2,1][..]]{
   assert!(owner.prepare_graph_order_as(&[3,2,1,0],invalid,&[1,0],positions.clone(),loops.clone(),Uuid::new_v4()).is_err());
   assert_eq!(owner.mesh(),&before);assert_eq!(owner.version,before_version);assert_eq!(owner.history_counts(),history);assert_eq!(budget.snapshot().total(),with_stale);assert!(view.check().is_ok());
  }
  owner.prepare_graph_order_as(&[3,2,1,0],&[4,3,2,1,0],&[1,0],positions,loops,Uuid::new_v4()).unwrap().apply();
  assert!(view.check().is_err());assert_eq!(owner.mesh().edges[4].attrs["opaque"].as_array().unwrap().as_ptr(),payload);
  let current=owner.mesh().clone();assert!(owner.commit(stale).is_err());assert_eq!(owner.mesh(),&current);assert_eq!(owner.history_counts(),history);assert_eq!(budget.snapshot().total(),charge);
  let view=owner.registry.view();assert_eq!(view.vertex(&crate::journal::SourceKey::new(DVec3::ZERO)),Some(3));assert_eq!(view.edge(&(2,3)),Some((4,3,2)));
 }
}

/// Exclusive prepared packed replacement: discard leaves semantic state,
/// chronological history and source accelerator revision untouched.
pub struct PreparedPackedReplacement<'a>{owner:&'a mut JournalOwner,mesh:Mesh,registry:Registry,state:Uuid,next_version:u64,_preparation:Lease,history:Option<bool>}
impl PreparedPackedReplacement<'_>{
    pub fn apply(self)->Mesh{
        self.owner.registry.invalidate();
        let old_mesh=std::mem::replace(&mut self.owner.mesh,self.mesh);
        let _old_registry=std::mem::replace(&mut self.owner.registry,self.registry);
        self.owner.state=self.state;
        self.owner.version=self.next_version;
        if let Some(redo)=self.history{
            if redo{let command=self.owner.redo.pop().expect("prepared redo command");self.owner.redo_bytes-=command.bytes;self.owner.undo_bytes+=command.bytes;self.owner.undo.push(command);}
            else{let command=self.owner.undo.pop().expect("prepared undo command");self.owner.undo_bytes-=command.bytes;self.owner.redo_bytes+=command.bytes;self.owner.redo.push(command);}
        }
        old_mesh
    }
}
#[cfg(test)]mod packed_replacement_tests{
 use super::*;
 fn initial()->Mesh{let mut m=Mesh::new();m.add_vertex(DVec3::ZERO);m.add_vertex(DVec3::X);m.add_edge(0,1);m}
 fn dropped_edge()->Mesh{let mut m=initial();m.edges.clear();for v in &mut m.vertices{v.edges.clear();}m}
 #[test]fn prepared_replacement_refusals_discard_and_chronological_inverse(){
  let budget=OwnerBudget::new(None);let mut o=JournalOwner::new_with_budget(initial(),budget.clone()).unwrap();
  let mut t=o.try_begin().unwrap();t.add_edge(DVec3::X,DVec3::Y).unwrap();assert!(o.commit(t).unwrap());
  let mut t=o.try_begin().unwrap();t.add_edge(DVec3::Y,DVec3::Z).unwrap();assert!(o.commit(t).unwrap());assert!(o.undo().unwrap());
  let before=o.mesh().clone();let oldstate=o.state_token();let oldhistory=o.history_counts();assert_eq!(oldhistory,(1,1));let view=o.registry.view();let stale=o.try_begin().unwrap();
  let baseline=budget.snapshot().total();let target=Uuid::new_v4();
  let ticket=o.prepare_packed_replacement_as(dropped_edge(),target).unwrap();assert!(budget.snapshot().total()>baseline);drop(ticket);assert_eq!(o.mesh(),&before);assert_eq!(o.history_counts(),oldhistory);assert_eq!(budget.snapshot().total(),baseline);assert!(view.check().is_ok());
  budget.set_limit(Some(baseline));assert!(o.prepare_packed_replacement_as(dropped_edge(),target).is_err());budget.set_limit(None);assert_eq!(o.mesh(),&before);assert_eq!(o.history_counts(),oldhistory);assert_eq!(budget.snapshot().total(),baseline);
  let mut invalid=dropped_edge();invalid.edges.push(MeshEdge::new(0,99));assert!(o.prepare_packed_replacement_as(invalid,target).is_err());assert_eq!(o.mesh(),&before);assert_eq!(o.history_counts(),oldhistory);assert_eq!(budget.snapshot().total(),baseline);
  let parked=o.prepare_packed_replacement_as(dropped_edge(),target).unwrap().apply();assert_eq!(parked,before);assert!(view.check().is_err());assert!(o.commit(stale).is_err());assert_eq!(o.history_counts(),oldhistory);let installed=o.mesh().clone();let held_charge=budget.snapshot().total();let installed_state=o.state_token();assert!(o.undo().is_err());assert!(o.redo().is_err());assert_eq!(o.mesh(),&installed);assert_eq!(o.state_token(),installed_state);assert_eq!(o.history_counts(),oldhistory);assert_eq!(budget.snapshot().total(),held_charge);
  let _removed=o.prepare_packed_replacement_as(parked,oldstate).unwrap().apply();assert_eq!(o.mesh(),&before);assert!(o.undo().unwrap());assert_eq!(o.mesh().edges.len(),1);assert!(o.redo().unwrap());assert_eq!(o.mesh(),&before);drop(o);assert!(budget.snapshot().registry>0);drop(view);assert_eq!(budget.snapshot().total(),0);
 }
}

#[cfg(test)]mod edge_replacement_keys_tests{
 use super::*;
 #[test]fn raw_pose_replacement_keeps_source_key_and_foreign_uid_refuses(){
  let mut mesh=Mesh::new();mesh.add_vertex(DVec3::ZERO);mesh.add_vertex(DVec3::X);mesh.add_edge(0,1);let budget=OwnerBudget::new(None);let mut o=JournalOwner::new_with_budget(mesh,budget.clone()).unwrap();let state=Uuid::new_v4();let mut candidate=o.mesh().clone();candidate.vertices[0].position=DVec3::Y;
  let old=o.prepare_edge_replacement_as(candidate,state).unwrap().apply();let view=o.registry.view();assert_eq!(view.vertex(&crate::journal::SourceKey::new(DVec3::ZERO)),Some(0));assert_eq!(view.vertex(&crate::journal::SourceKey::new(DVec3::Y)),None);
  let mut foreign=o.mesh().clone();foreign.vertices[0].uid=Uuid::new_v4();let charge=budget.snapshot().total();assert!(o.prepare_edge_replacement_as(foreign,state).is_err());assert_eq!(budget.snapshot().total(),charge);assert_eq!(o.mesh().vertices[0].position,DVec3::Y);
  o.prepare_edge_replacement_as(old,state).unwrap().apply();assert!(view.check().is_err());drop(view);drop(o);assert_eq!(budget.snapshot().total(),0);
 }
}
