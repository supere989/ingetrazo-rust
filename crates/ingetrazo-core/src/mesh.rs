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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MeshEdge {
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MeshFace {
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
            interior: false,
            vertices,
            holes: Vec::new(),
            attrs: HashMap::new(),
            layer: None,
        }
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
        for vertex in &original.vertices {identities.entry(PointKey::from_vec3(vertex.position)).or_insert(vertex.uid);}
        for vertex in &mut self.vertices {if let Some(&uid)=identities.get(&PointKey::from_vec3(vertex.position)){vertex.uid=uid;}}
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
        for (id,&position) in positions.iter().enumerate(){index.insert(PointKey::from_vec3(position),id);}
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
        for (offset,&p) in positions.iter().enumerate(){if !p.is_finite(){return Err("Nonfinite spatial delta coordinate");}let key=PointKey::from_vec3(p);let id=base+offset;ordered.push(p);if let Some(&slot)=seen.get(&key){changes[slot].2=id;}else{seen.insert(key,changes.len());changes.push((key,self.spatial_index.get(&key).copied(),id));}}
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

    /// Rebuild the spatial lookup table (e.g. after deserialization).
    pub fn rebuild_spatial_index(&mut self) {
        self.spatial_index.clear();
        for (i, v) in self.vertices.iter().enumerate() {
            let key = PointKey::from_vec3(v.position);
            self.spatial_index.insert(key, i);
        }
    }

    /// Resolve the same existing endpoint that add_vertex would weld to.
    pub fn vertex_at_position(&self, p:DVec3)->Option<VertexId>{
        self.spatial_index.get(&PointKey::from_vec3(p)).copied()
    }

    /// Add a 3D position to the mesh, automatically welding to any coincident vertex.
    pub fn add_vertex(&mut self, p: DVec3) -> VertexId {
        let key = PointKey::from_vec3(p);
        if let Some(&existing) = self.spatial_index.get(&key) {
            return existing;
        }
        let id = self.vertices.len();
        self.vertices.push(MeshVertex::new(p));
        self.spatial_index.insert(key, id);
        id
    }

    /// Find an existing edge between vertices v0 and v1 regardless of order.
    pub fn find_edge(&self, v0: VertexId, v1: VertexId) -> Option<EdgeId> {
        if v0 == v1 || v0 >= self.vertices.len() {
            return None;
        }
        for &e_id in &self.vertices[v0].edges {
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
