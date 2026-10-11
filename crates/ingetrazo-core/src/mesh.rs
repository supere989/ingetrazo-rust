// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! Shared-vertex, non-manifold connectivity mesh.
//!
//! Level B+C topology engine:
//! - Vertices are shared and automatically welded by spatial hash key.
//! - Edges maintain radial lists of incident faces (supporting non-manifold 0, 1, 2, or more incident faces).
//! - Moving a vertex updates all incident edges and faces automatically.

use std::collections::HashMap;
use glam::DVec3;
use serde::{Deserialize, Serialize};

use crate::math::{newell_vector, PointKey, EPSILON};
use crate::triangulate::{triangulate, Tri3};

// Source Qt-authored keys use Python's ties-even quantization. Native
// doubles keep the existing key extension; neither branch changes payloads.
fn mesh_weld_key(p:DVec3)->PointKey {
    if p.is_finite()&&p.as_vec3().as_dvec3()==p {
        PointKey((p.x*crate::math::KEY_SCALE).round_ties_even()as i64,
                 (p.y*crate::math::KEY_SCALE).round_ties_even()as i64,
                 (p.z*crate::math::KEY_SCALE).round_ties_even()as i64)
    }else{PointKey::from_vec3(p)}
}

pub type VertexId = usize;
pub type EdgeId = usize;
pub type FaceId = usize;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MeshVertex {
    #[serde(default="uuid::Uuid::new_v4")]
    pub uid:uuid::Uuid,
    pub position: DVec3,
    pub edges: Vec<EdgeId>,
}

impl MeshVertex {
    pub fn new(position: DVec3) -> Self {
        Self {
            uid:uuid::Uuid::new_v4(),
            position,
            edges: Vec::new(),
        }
    }
}

fn fresh_history_identity() -> u64 {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    NEXT.fetch_update(std::sync::atomic::Ordering::Relaxed, std::sync::atomic::Ordering::Relaxed, |id| id.checked_add(1)).expect("runtime mesh identity exhausted")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshEdge {
    /// Runtime object identity, preserved by identity-owning history snapshots.
    #[serde(skip, default = "fresh_history_identity")]
    pub history_id: u64,
    /// Opaque native edge metadata follows edge identity through edits.
    #[serde(default)]
    pub attrs: std::collections::HashMap<String, serde_json::Value>,
    pub v0: VertexId,
    pub v1: VertexId,
    pub faces: Vec<FaceId>,
    pub soft: bool,
    pub curve_id: Option<usize>,
    pub layer: Option<String>,
    pub hidden: bool,
}

impl MeshEdge {
    pub fn new(v0: VertexId, v1: VertexId) -> Self {
        Self {
            history_id: fresh_history_identity(),
            attrs: std::collections::HashMap::new(),
            v0,
            v1,
            faces: Vec::new(),
            soft: false,
            curve_id: None,
            layer: None,
            hidden: false,
        }
    }

    #[inline]
    pub fn contains_vertex(&self, v: VertexId) -> bool {
        self.v0 == v || self.v1 == v
    }

    #[inline]
    pub fn other_vertex(&self, v: VertexId) -> Option<VertexId> {
        if self.v0 == v {
            Some(self.v1)
        } else if self.v1 == v {
            Some(self.v0)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshFace {
    /// Runtime object identity; persisted data and semantic equality omit it.
    #[serde(skip, default = "fresh_history_identity")]
    pub history_id: u64,
    /// Runtime partition classification. The original native file does not
    /// persist this derived orientation state.
    #[serde(skip)]
    pub interior: bool,
    pub vertices: Vec<VertexId>,
    pub holes: Vec<Vec<VertexId>>,
    pub attrs: HashMap<String, serde_json::Value>,
    pub layer: Option<String>,
}

impl MeshFace {
    pub fn new(vertices: Vec<VertexId>) -> Self {
        Self {
            history_id: fresh_history_identity(),
            interior: false,
            vertices,
            holes: Vec::new(),
            attrs: HashMap::new(),
            layer: None,
        }
    }
}

impl PartialEq for MeshEdge {
    fn eq(&self, other: &Self) -> bool {
        let Self { history_id: _, attrs, v0, v1, faces, soft, curve_id, layer, hidden } = self;
        attrs == &other.attrs && v0 == &other.v0 && v1 == &other.v1 && faces == &other.faces && soft == &other.soft && curve_id == &other.curve_id && layer == &other.layer && hidden == &other.hidden
    }
}
impl PartialEq for MeshFace {
    fn eq(&self, other: &Self) -> bool {
        let Self { history_id: _, interior, vertices, holes, attrs, layer } = self;
        interior == &other.interior && vertices == &other.vertices && holes == &other.holes && attrs == &other.attrs && layer == &other.layer
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Mesh {
    pub vertices: Vec<MeshVertex>,
    pub edges: Vec<MeshEdge>,
    pub faces: Vec<MeshFace>,
    #[serde(skip)]
    spatial_index: HashMap<PointKey, VertexId>,
}

/// Prepared replacement lookup. All allocation occurs before publication;
/// coordinates bind this index to the exact ordered vertex positions.
pub struct PreparedSpatialDelta { base:usize, positions:Vec<DVec3>, changes:Vec<(PointKey,Option<VertexId>,VertexId)> }
impl PreparedSpatialDelta {pub fn retained_bytes(&self)->usize{self.positions.capacity()*std::mem::size_of::<DVec3>()+self.changes.capacity()*std::mem::size_of::<(PointKey,Option<VertexId>,VertexId)>()}}
pub struct PreparedSpatialIndex { positions:Vec<DVec3>, index:HashMap<PointKey,VertexId> }
impl PreparedSpatialIndex {
    /// Immutable ordered positions used to validate virtual changed-row publication
    /// before any live mesh write. Borrowing this slice performs no allocation.
    pub fn positions(&self)->&[DVec3]{&self.positions}
}

impl Default for Mesh {
    fn default() -> Self {
        Self::new()
    }
}

impl Mesh {
    /// Entry capacity for logical payload accounting; excludes hash-table control/allocator overhead.
    pub fn spatial_index_capacity(&self) -> usize { self.spatial_index.capacity() }

    pub fn new() -> Self {
        Self {
            vertices: Vec::new(),
            edges: Vec::new(),
            faces: Vec::new(),
            spatial_index: HashMap::new(),
        }
    }

    /// Retain existing vertex identity when a topology rebuild keeps its position.
    /// New cut/extrusion stations receive fresh identities from add_vertex.
    pub fn preserve_vertex_uids_from(&mut self, original:&Mesh) {
        let mut identities=HashMap::new();
        for vertex in &original.vertices {identities.entry(mesh_weld_key(vertex.position)).or_insert(vertex.uid);}
        for vertex in &mut self.vertices {if let Some(&uid)=identities.get(&mesh_weld_key(vertex.position)){vertex.uid=uid;}}
    }

    pub fn clear(&mut self) {
        self.vertices.clear();
        self.edges.clear();
        self.faces.clear();
        self.spatial_index.clear();
    }

    pub fn prepare_spatial_index(positions:&[DVec3])->Result<PreparedSpatialIndex,&'static str>{
        if positions.iter().any(|p|!p.is_finite()){return Err("Nonfinite prepared index coordinate");}
        let mut ordered=Vec::new();ordered.try_reserve_exact(positions.len()).map_err(|_|"Prepared coordinate allocation refused")?;
        ordered.extend_from_slice(positions);
        let mut index=HashMap::new();index.try_reserve(positions.len()).map_err(|_|"Prepared index allocation refused")?;
        for (id,&position) in positions.iter().enumerate(){index.insert(mesh_weld_key(position),id);}
        Ok(PreparedSpatialIndex{positions:ordered,index})
    }
    /// Allocation-free validation and swap; a foreign/stale preparation cannot
    /// replace the lookup. Duplicate native keys retain last-vertex semantics.
    pub fn install_prepared_spatial_index(&mut self,prepared:PreparedSpatialIndex)->Result<(),&'static str>{
        if self.vertices.len()!=prepared.positions.len()||self.vertices.iter().zip(&prepared.positions).any(|(v,p)|v.position!=*p){return Err("Prepared index does not match vertices");}
        self.spatial_index=prepared.index;Ok(())
    }

    /// Private append-owner preparation: reserves before publication and records
    /// predecessor mappings so native-key collisions can be reversed exactly.
    /// Caller must preserve exclusive ownership and the untouched prefix.
    pub fn prepare_spatial_append(&mut self,positions:&[DVec3])->Result<PreparedSpatialDelta,&'static str>{
        let base=self.vertices.len();base.checked_add(positions.len()).ok_or("Spatial append extent overflow")?;
        let mut ordered=Vec::new();ordered.try_reserve_exact(positions.len()).map_err(|_|"Spatial delta coordinates refused")?;
        let mut changes:Vec<(PointKey,Option<VertexId>,VertexId)>=Vec::new();changes.try_reserve_exact(positions.len()).map_err(|_|"Spatial delta keys refused")?;
        let mut seen=HashMap::<PointKey,usize>::new();seen.try_reserve(positions.len()).map_err(|_|"Spatial delta key planning refused")?;
        for (offset,&p) in positions.iter().enumerate(){if !p.is_finite(){return Err("Nonfinite spatial delta coordinate");}let key=mesh_weld_key(p);let id=base+offset;ordered.push(p);if let Some(&slot)=seen.get(&key){changes[slot].2=id;}else{seen.insert(key,changes.len());changes.push((key,self.spatial_index.get(&key).copied(),id));}}
        self.spatial_index.try_reserve(changes.len()).map_err(|_|"Spatial delta index reservation refused")?;
        Ok(PreparedSpatialDelta{base,positions:ordered,changes})
    }
    pub fn validate_spatial_append(&self,delta:&PreparedSpatialDelta)->Result<(),&'static str>{
        if self.vertices.len()!=delta.base||delta.changes.iter().any(|(k,old,_)|self.spatial_index.get(k).copied()!=*old){return Err("Stale spatial append predecessor");}Ok(())
    }
    pub fn install_spatial_append(&mut self,delta:&PreparedSpatialDelta)->Result<(),&'static str>{
        if self.vertices.len()!=delta.base+delta.positions.len()||self.vertices[delta.base..].iter().zip(&delta.positions).any(|(v,p)|v.position!=*p)||delta.changes.iter().any(|(k,old,_)|self.spatial_index.get(k).copied()!=*old){return Err("Stale spatial append publication");}
        for &(key,_,id) in &delta.changes{self.spatial_index.insert(key,id);}Ok(())
    }
    /// Validate before any caller semantic writes. After success, restoration
    /// performs no allocations; the exclusive owner immediately truncates tail.
    pub fn restore_spatial_append(&mut self,delta:&PreparedSpatialDelta)->Result<(),&'static str>{
        if self.vertices.len()!=delta.base+delta.positions.len()||self.vertices[delta.base..].iter().zip(&delta.positions).any(|(v,p)|v.position!=*p)||delta.changes.iter().any(|(k,_,id)|self.spatial_index.get(k)!=Some(id)){return Err("Stale spatial truncate publication");}
        for &(key,old,_) in &delta.changes{if let Some(id)=old{self.spatial_index.insert(key,id);}else{self.spatial_index.remove(&key);}}Ok(())
    }
    pub fn reserve_spatial_append(&mut self,delta:&PreparedSpatialDelta)->Result<(),&'static str>{self.validate_spatial_append(delta)?;self.spatial_index.try_reserve(delta.changes.len()).map_err(|_|"Spatial redo reservation refused")}

    /// Publish a frozen transform in two passes over only its owned vertices.
    /// Retiring old keys before assigning new ones preserves both endpoints
    /// when a mirror exchanges their cells. No model-wide rebuild is needed.
    pub fn place_vertices<I>(&mut self, positions:I)->bool
    where I:Iterator<Item=(VertexId,u128,DVec3)>+Clone {
        let mut changed=false;
        for (id,identity,position) in positions.clone() {
            let Some(vertex)=self.vertices.get(id).filter(|v|v.uid.as_u128()==identity)else{continue;};
            changed|=vertex.position!=position;
            let old=mesh_weld_key(vertex.position);let new=mesh_weld_key(position);
            if old!=new&&self.spatial_index.get(&old)==Some(&id){self.spatial_index.remove(&old);}
        }
        for (id,identity,position) in positions {
            let Some(vertex)=self.vertices.get_mut(id).filter(|v|v.uid.as_u128()==identity)else{continue;};
            vertex.position=position;self.spatial_index.entry(mesh_weld_key(position)).or_insert(id);
        }
        changed
    }

    /// Rebuild the spatial lookup table (e.g. after deserialization).
    pub fn rebuild_spatial_index(&mut self) {
        self.spatial_index.clear();
        for (i, v) in self.vertices.iter().enumerate() {
            let key = mesh_weld_key(v.position);
            self.spatial_index.insert(key, i);
        }
    }

    /// Refresh derived endpoint and radial indices after row compaction.
    /// Entity records and runtime identities are not recreated.
    pub fn rebuild_incidence(&mut self) {
        let mut pairs = HashMap::with_capacity(self.edges.len());
        for vertex in &mut self.vertices { vertex.edges.clear(); }
        for (id, edge) in self.edges.iter_mut().enumerate() {
            edge.faces.clear();
            pairs.insert((edge.v0.min(edge.v1), edge.v0.max(edge.v1)), id);
            if let Some(vertex) = self.vertices.get_mut(edge.v0) { vertex.edges.push(id); }
            if let Some(vertex) = self.vertices.get_mut(edge.v1) { vertex.edges.push(id); }
        }
        for (face_id, face) in self.faces.iter().enumerate() {
            for ring in std::iter::once(&face.vertices).chain(&face.holes) {
                if ring.len() < 2 { continue; }
                for i in 0..ring.len() {
                    let a = ring[i]; let b = ring[(i + 1) % ring.len()];
                    if let Some(&edge) = pairs.get(&(a.min(b), a.max(b))) {
                        if self.edges[edge].faces.last() != Some(&face_id) {
                            self.edges[edge].faces.push(face_id);
                        }
                    }
                }
            }
        }
    }

    /// Resolve the same existing endpoint that add_vertex would weld to.
    pub fn vertex_at_position(&self, p:DVec3)->Option<VertexId>{
        let key=mesh_weld_key(p);
        if let Some(&id)=self.spatial_index.get(&key){return Some(id);}
        if !p.is_finite(){return None;}
        for dx in -1i64..=1 {for dy in -1i64..=1 {for dz in -1i64..=1 {
            if dx==0&&dy==0&&dz==0{continue;}
            let Some(x)=key.0.checked_add(dx)else{continue};
            let Some(y)=key.1.checked_add(dy)else{continue};
            let Some(z)=key.2.checked_add(dz)else{continue};
            if let Some(&id)=self.spatial_index.get(&PointKey(x,y,z)) {
                let q=self.vertices[id].position;
                // Qt-authored endpoints use the source QVector3D distance law.
                // Genuine native doubles keep their distance and payload precision.
                let close=if p.as_vec3().as_dvec3()==p&&q.as_vec3().as_dvec3()==q {
                    let d=p.as_vec3()-q.as_vec3();
                    f64::from(d.x*d.x+d.y*d.y+d.z*d.z)<1e-8
                }else{(p-q).length_squared()<1e-8};
                if close{return Some(id);}
            }
        }}}
        None
    }

    /// Add a 3D position to the mesh, automatically welding to any coincident vertex.
    pub fn add_vertex(&mut self, p: DVec3) -> VertexId {
        let key = mesh_weld_key(p);
        if let Some(existing) = self.vertex_at_position(p) {
            return existing;
        }
        let id = self.vertices.len();
        self.vertices.push(MeshVertex::new(p));
        self.spatial_index.insert(key, id);
        id
    }

    /// Find an existing edge between vertices v0 and v1 regardless of order.
    pub fn find_edge(&self, v0: VertexId, v1: VertexId) -> Option<EdgeId> {
        if v0 == v1 || v0 >= self.vertices.len() || v1 >= self.vertices.len() {
            return None;
        }
        let small=if self.vertices[v0].edges.len()<=self.vertices[v1].edges.len(){v0}else{v1};
        for &e_id in &self.vertices[small].edges {
            let e = &self.edges[e_id];
            if (e.v0 == v0 && e.v1 == v1) || (e.v0 == v1 && e.v1 == v0) {
                return Some(e_id);
            }
        }
        None
    }

    /// Add an edge between two vertices. Reuses existing edge if already present.
    pub fn add_edge(&mut self, v0: VertexId, v1: VertexId) -> Option<EdgeId> {
        if v0 == v1 || v0 >= self.vertices.len() || v1 >= self.vertices.len() {
            return None;
        }
        if let Some(existing) = self.find_edge(v0, v1) {
            return Some(existing);
        }
        let id = self.edges.len();
        self.edges.push(MeshEdge::new(v0, v1));
        self.vertices[v0].edges.push(id);
        self.vertices[v1].edges.push(id);
        Some(id)
    }

    /// Source shared-edge split: all radial faces reuse their records; the
    /// removed edge is replaced by ordinary constructor subedges, or already
    /// existing subedges. Admission finishes before any semantic mutation.
    pub fn split_edge(&mut self, edge_id:EdgeId, position:DVec3)->Result<(EdgeId,EdgeId),String>{
        if !position.is_finite(){return Err("Nonfinite split position".into());}
        let edge=self.edges.get(edge_id).ok_or("Missing split edge")?;
        let (a,b)=(edge.v0,edge.v1);
        if a>=self.vertices.len()||b>=self.vertices.len()||a==b{return Err("Invalid split endpoints".into());}
        let existing=self.vertex_at_position(position);
        if existing==Some(a)||existing==Some(b){return Ok((edge_id,edge_id));}
        let mut incident=Vec::new();incident.try_reserve_exact(edge.faces.len()).map_err(|_|"Split radial admission refused")?;
        incident.extend_from_slice(&edge.faces);
        // Record only the source's first matching ring/station per incident face.
        let mut seen=std::collections::HashSet::new();seen.try_reserve(incident.len()).map_err(|_|"Split radial identity admission refused")?;
        let mut inserts=Vec::new();inserts.try_reserve_exact(incident.len()).map_err(|_|"Split ring admission refused")?;
        for &face_id in &incident {
            if !seen.insert(face_id){continue;}
            let face=self.faces.get(face_id).ok_or("Invalid split radial face")?;
            let station=std::iter::once(&face.vertices).chain(&face.holes).enumerate().find_map(|(ring_id,ring)|{
                if ring.iter().any(|&v|v>=self.vertices.len()){return Some(Err("Invalid split ring vertex"));}
                (0..ring.len()).find(|&i|{let x=ring[i];let y=ring[(i+1)%ring.len()];(x==a&&y==b)||(x==b&&y==a)}).map(|i|Ok((ring_id,i+1)))
            }).transpose()?.ok_or("Split radial face lacks endpoint boundary")?;
            inserts.push((face_id,station.0,station.1));
        }
        if existing.is_none(){self.vertices.try_reserve(1).map_err(|_|"Split vertex admission refused")?;self.spatial_index.try_reserve(1).map_err(|_|"Split index admission refused")?;}
        self.edges.try_reserve(2).map_err(|_|"Split edge admission refused")?;
        for id in [Some(a),Some(b),existing].into_iter().flatten(){self.vertices[id].edges.try_reserve(2).map_err(|_|"Split incidence admission refused")?;}
        for &(f,r,_) in &inserts {let face=&mut self.faces[f];let ring=if r==0{&mut face.vertices}else{&mut face.holes[r-1]};ring.try_reserve(1).map_err(|_|"Split face admission refused")?;}
        // Existing welded subedges must retain their own metadata and identities.
        for &v in [a,b].iter(){if let Some(mid)=existing{if let Some(id)=self.find_edge(v,mid){self.edges[id].faces.try_reserve(incident.len()).map_err(|_|"Split reused radial admission refused")?;}}}
        let mut fresh0=MeshEdge::new(a,existing.unwrap_or(self.vertices.len()));
        let mut fresh1=MeshEdge::new(existing.unwrap_or(self.vertices.len()),b);
        fresh0.faces.try_reserve_exact(incident.len()).map_err(|_|"Split radial allocation refused")?;
        fresh1.faces.try_reserve_exact(incident.len()).map_err(|_|"Split radial allocation refused")?;
        let mut fresh_vertex=MeshVertex::new(position);fresh_vertex.edges.try_reserve_exact(2).map_err(|_|"Split vertex links refused")?;
        let mid=if let Some(id)=existing{id}else{let id=self.vertices.len();self.vertices.push(fresh_vertex);self.spatial_index.insert(mesh_weld_key(position),id);id};
        self.edges.remove(edge_id);
        for vertex in &mut self.vertices {vertex.edges.retain(|&id|id!=edge_id);for id in &mut vertex.edges{if *id>edge_id{*id-=1;}}}
        let link=|mesh:&mut Mesh,v0,v1,fresh:MeshEdge|{if let Some(id)=mesh.find_edge(v0,v1){id}else{let id=mesh.edges.len();mesh.edges.push(fresh);mesh.vertices[v0].edges.push(id);mesh.vertices[v1].edges.push(id);id}};
        let e0=link(self,a,mid,fresh0);let e1=link(self,mid,b,fresh1);
        for (f,r,i) in inserts {let face=&mut self.faces[f];let ring=if r==0{&mut face.vertices}else{&mut face.holes[r-1]};ring.insert(i,mid);for e in [e0,e1]{if !self.edges[e].faces.contains(&f){self.edges[e].faces.push(f);}}}
        Ok((e0,e1))
    }

    /// Add a face defined by an ordered loop of vertex IDs and optional holes.
    /// Also ensures all boundary and hole edges are added to the mesh.
    pub fn add_face(&mut self, loop_verts: Vec<VertexId>, holes: Vec<Vec<VertexId>>) -> Option<FaceId> {
        if loop_verts.len() < 3 {
            return None;
        }

        // Add boundary edges
        let n = loop_verts.len();
        let mut boundary_edges = Vec::with_capacity(n);
        for i in 0..n {
            let v0 = loop_verts[i];
            let v1 = loop_verts[(i + 1) % n];
            if let Some(e) = self.add_edge(v0, v1) {
                boundary_edges.push(e);
            }
        }

        // Add hole edges
        let mut hole_edges = Vec::new();
        for h in &holes {
            let hn = h.len();
            for i in 0..hn {
                let v0 = h[i];
                let v1 = h[(i + 1) % hn];
                if let Some(e) = self.add_edge(v0, v1) {
                    hole_edges.push(e);
                }
            }
        }

        let face_id = self.faces.len();
        self.faces.push(MeshFace {
            history_id: fresh_history_identity(),
            interior: false,
            vertices: loop_verts,
            holes,
            attrs: HashMap::new(),
            layer: None,
        });

        // Register incident face on all boundary and hole edges
        for e in boundary_edges.into_iter().chain(hole_edges) {
            if !self.edges[e].faces.contains(&face_id) {
                self.edges[e].faces.push(face_id);
            }
        }

        Some(face_id)
    }

    /// Compute Newell normal of a face.
    pub fn face_normal(&self, face_id: FaceId) -> DVec3 {
        let f = &self.faces[face_id];
        let positions: Vec<DVec3> = f
            .vertices
            .iter()
            .map(|&v| self.vertices[v].position)
            .collect();
        let n = newell_vector(&positions);
        if n.length_squared() < EPSILON {
            DVec3::Z
        } else {
            n.normalize()
        }
    }

    /// Compute outer area of a face.
    pub fn face_area(&self, face_id: FaceId) -> f64 {
        let f = &self.faces[face_id];
        let positions: Vec<DVec3> = f
            .vertices
            .iter()
            .map(|&v| self.vertices[v].position)
            .collect();
        let outer=0.5 * newell_vector(&positions).length();
        let holes:f64=f.holes.iter().map(|ring| {
            let positions:Vec<_>=ring.iter().map(|&v|self.vertices[v].position).collect();
            0.5*newell_vector(&positions).length()
        }).sum();
        (outer-holes).max(0.)
    }

    /// Compute centroid of a face.
    pub fn face_centroid(&self, face_id: FaceId) -> DVec3 {
        let f = &self.faces[face_id];
        if f.vertices.is_empty() {
            return DVec3::ZERO;
        }
        let sum: DVec3 = f
            .vertices
            .iter()
            .map(|&v| self.vertices[v].position)
            .sum();
        sum / (f.vertices.len() as f64)
    }

    /// Triangulate a face into 3D triangles.
    pub fn triangulate_face(&self, face_id: FaceId) -> Vec<Tri3> {
        let f = &self.faces[face_id];
        let outer: Vec<DVec3> = f
            .vertices
            .iter()
            .map(|&v| self.vertices[v].position)
            .collect();
        let holes: Vec<Vec<DVec3>> = f
            .holes
            .iter()
            .map(|h| h.iter().map(|&v| self.vertices[v].position).collect())
            .collect();
        triangulate(&outer, &holes, Some(self.face_normal(face_id)))
    }

    /// Check if the mesh is manifold closed solid (watertight):
    /// - Every edge borders exactly two faces
    /// - At least 4 faces
    pub fn is_manifold_solid(&self) -> bool {
        if self.faces.iter().filter(|f| !f.interior).count() < 4 || self.edges.is_empty() {
            return false;
        }
        for e in &self.edges {
            if e.faces.iter().filter(|&&i| i < self.faces.len() && !self.faces[i].interior).count() != 2 {
                return false;
            }
        }
        true
    }

    /// Compute volume enclosed by the mesh using signed tetrahedrons.
    pub fn volume(&self) -> Option<f64> {
        if !self.is_manifold_solid() {
            return None;
        }
        let mut total_vol = 0.0;
        for face_id in 0..self.faces.len() {
            if self.faces[face_id].interior { continue; }
            let tris = self.triangulate_face(face_id);
            for (p0, p1, p2) in tris {
                // Signed tetrahedron volume: (p0 . (p1 x p2)) / 6.0
                total_vol += p0.dot(p1.cross(p2)) / 6.0;
            }
        }
        let v = total_vol.abs();
        if v > 1e-12 {
            Some(v)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mesh_vertex_welding() {
        let mut mesh = Mesh::new();
        let v0 = mesh.add_vertex(DVec3::new(1.0, 2.0, 3.0));
        let v1 = mesh.add_vertex(DVec3::new(1.00001, 2.00002, 3.00001));
        assert_eq!(v0, v1);
        assert_eq!(mesh.vertices.len(), 1);
    }

    #[test]
    fn test_mesh_cube_volume() {
        let mut mesh = Mesh::new();
        // Cube corners [0, 1]
        let v000 = mesh.add_vertex(DVec3::new(0.0, 0.0, 0.0));
        let v100 = mesh.add_vertex(DVec3::new(1.0, 0.0, 0.0));
        let v110 = mesh.add_vertex(DVec3::new(1.0, 1.0, 0.0));
        let v010 = mesh.add_vertex(DVec3::new(0.0, 1.0, 0.0));
        let v001 = mesh.add_vertex(DVec3::new(0.0, 0.0, 1.0));
        let v101 = mesh.add_vertex(DVec3::new(1.0, 0.0, 1.0));
        let v111 = mesh.add_vertex(DVec3::new(1.0, 1.0, 1.0));
        let v011 = mesh.add_vertex(DVec3::new(0.0, 1.0, 1.0));

        // 6 faces
        mesh.add_face(vec![v000, v010, v110, v100], vec![]); // bottom (-Z)
        mesh.add_face(vec![v001, v101, v111, v011], vec![]); // top (+Z)
        mesh.add_face(vec![v000, v100, v101, v001], vec![]); // front (-Y)
        mesh.add_face(vec![v100, v110, v111, v101], vec![]); // right (+X)
        mesh.add_face(vec![v110, v010, v011, v111], vec![]); // back (+Y)
        mesh.add_face(vec![v010, v000, v001, v011], vec![]); // left (-X)

        assert!(mesh.is_manifold_solid());
        let vol = mesh.volume().unwrap();
        assert!((vol - 1.0).abs() < 1e-4);
    }
}
