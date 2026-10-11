//! Standalone proper-ring face adapter. Not full Python Mesh API.
use crate::{
    owner::{JournalOwner, Transaction},
    owner_budget::{Kind, Lease, OwnerBudget},
};
use glam::DVec3;
use ingetrazo_core::mesh::{Mesh, MeshFace};
use std::collections::HashMap;
struct Draft {
    face: MeshFace,
    edges: Vec<usize>,
}
pub struct FaceTransaction {
    edge: Transaction,
    drafts: Vec<Draft>,
    base_faces: usize,
    base_edges: usize,
    charge: Lease,
    budget: OwnerBudget,
    failed: bool,
    corners: usize,
    attribute_bytes: usize,
    attribute_charge: Lease,
}
struct Command {
    base_faces: usize,
    base_edges: usize,
    faces: Vec<MeshFace>,
    face_count: usize,
    corners: usize,
    incidence: Vec<(usize, Vec<usize>)>,
    _charge: Lease,
}
pub struct FaceOwner {
    owner: JournalOwner,
    undo: Vec<Command>,
    redo: Vec<Command>,
}
/// Exclusive prepared face/edge publication; drop discards all staged records.
pub struct PreparedFaces<'a> {
    edge: crate::owner::PreparedAppend<'a>,
    undo: &'a mut Vec<Command>,
    redo: &'a mut Vec<Command>,
    faces: Vec<MeshFace>,
    prepared_new: Vec<Vec<usize>>,
    radial: Vec<(usize, Vec<usize>)>,
    command: Command,
    _preparation: Lease,
    _radial_charge: Lease,
}
impl PreparedFaces<'_> {
    pub fn with_existing_fields(
        mut self,
        positions: Vec<DVec3>,
        loops: Vec<(Vec<usize>, Vec<Vec<usize>>)>,
    ) -> Result<Self, String> {
        self.edge = self.edge.with_existing_fields(positions, loops)?;
        Ok(self)
    }
    pub fn apply(self) {
        let Self {
            edge,
            undo,
            redo,
            faces,
            prepared_new,
            radial,
            mut command,
            _preparation,
            _radial_charge,
        } = self;
        let owner = edge.apply();
        let (storage, edges) = owner.face_storage();
        for (eid, ids) in &radial {
            if *eid < command.base_edges {
                edges[*eid].faces.extend(ids);
            }
        }
        for (offset, ids) in prepared_new.into_iter().enumerate() {
            edges[command.base_edges + offset].faces = ids;
        }
        storage.extend(faces);
        command.incidence = radial;
        redo.clear();
        undo.push(command);
        drop(_preparation);
        drop(_radial_charge);
    }
}
impl FaceOwner {
    pub fn new(mesh: Mesh) -> Result<Self, String> {
        Self::new_with_budget(mesh, OwnerBudget::new(None))
    }
    pub fn new_with_budget(mesh: Mesh, budget: OwnerBudget) -> Result<Self, String> {
        Ok(Self {
            owner: JournalOwner::new_with_budget(mesh, budget)?,
            undo: vec![],
            redo: vec![],
        })
    }
    pub fn mesh(&self) -> &Mesh {
        self.owner.mesh()
    }
    pub(crate) fn into_mesh(self) -> Mesh {
        self.owner.into_mesh()
    }
    pub fn budget(&self) -> OwnerBudget {
        self.owner.budget()
    }
    pub fn history_counts(&self) -> (usize, usize) {
        (self.undo.len(), self.redo.len())
    }
    pub fn begin(&self) -> Result<FaceTransaction, String> {
        Ok(FaceTransaction {
            edge: self.owner.try_begin()?,
            drafts: vec![],
            base_faces: self.mesh().faces.len(),
            base_edges: self.mesh().edges.len(),
            charge: self.budget().reserve(Kind::Staging, 0)?,
            budget: self.budget(),
            failed: false,
            corners: 0,
            attribute_bytes: 0,
            attribute_charge: self.budget().reserve(Kind::Staging, 0)?,
        })
    }
    pub fn commit(&mut self, t: FaceTransaction) -> Result<bool, String> {
        let Some(prepared) = self.prepare(t)? else {
            return Ok(false);
        };
        prepared.apply();
        Ok(true)
    }
    pub fn prepare(&mut self, mut t: FaceTransaction) -> Result<Option<PreparedFaces<'_>>, String> {
        if t.failed || t.edge.journal.admission_failed {
            return Err("Face transaction refused; staged work discarded".into());
        }
        if t.drafts.is_empty() {
            return Ok(None);
        }
        if t.base_faces != self.mesh().faces.len() || t.base_edges != self.mesh().edges.len() {
            return Err("Stale face transaction".into());
        }
        let prep = self
            .budget()
            .reserve(Kind::Preparation, face_bound(t.corners, t.drafts.len())?)?;
        let mut faces = Vec::new();
        faces
            .try_reserve_exact(t.drafts.len())
            .map_err(|_| "Face publication records refused")?;
        let mut incidence = HashMap::<usize, Vec<usize>>::new();
        incidence
            .try_reserve(t.corners)
            .map_err(|_| "Face incidence planning refused")?;
        for (offset, draft) in t.drafts.into_iter().enumerate() {
            let fid = t
                .base_faces
                .checked_add(offset)
                .ok_or("Face extent overflow")?;
            for eid in draft.edges {
                let list = incidence.entry(eid).or_default();
                list.try_reserve(1)
                    .map_err(|_| "Face incidence planning refused")?;
                list.push(fid);
            }
            faces.push(draft.face);
        }
        let mut retained = Vec::new();
        retained
            .try_reserve_exact(faces.len())
            .map_err(|_| "Face command slots refused")?;
        let face_count = faces.len();
        let mut radial = Vec::new();
        radial
            .try_reserve_exact(incidence.len())
            .map_err(|_| "Face radial history refused")?;
        for item in incidence {
            radial.push(item);
        }
        radial.sort_unstable_by_key(|x| x.0);
        let bytes = face_payload(&faces, &radial)?
            .checked_add(
                retained
                    .capacity()
                    .checked_mul(std::mem::size_of::<MeshFace>())
                    .ok_or("Face slot overflow")?,
            )
            .ok_or("Face history byte overflow")?;
        let mut charge = self.budget().reserve(
            Kind::History,
            bytes
                .checked_sub(t.attribute_bytes)
                .ok_or("Face attribute charge mismatch")?,
        )?;
        t.attribute_charge.reclassify(Kind::History)?;
        charge.merge(t.attribute_charge)?;
        self.undo
            .try_reserve(1)
            .map_err(|_| "Face history refused")?;
        {
            let (storage, edges) = self.owner.face_storage();
            storage
                .try_reserve(faces.len())
                .map_err(|_| "Live face reservation refused")?;
            for (eid, ids) in &radial {
                if *eid < t.base_edges {
                    edges[*eid]
                        .faces
                        .try_reserve(ids.len())
                        .map_err(|_| "Live radial reservation refused")?;
                }
            }
        }
        // New edge radial vectors are reserved after private edge publication only
        // if preprepared storage is attached; prepare all new vectors here instead.
        let edge_total = t
            .base_edges
            .checked_add(t.edge.journal.added_edges.len())
            .ok_or("Edge extent overflow")?;
        let radial_bytes = (edge_total - t.base_edges)
            .checked_mul(std::mem::size_of::<Vec<usize>>())
            .and_then(|n| {
                radial
                    .iter()
                    .filter(|x| x.0 >= t.base_edges)
                    .try_fold(n, |n, x| {
                        x.1.len()
                            .checked_mul(std::mem::size_of::<usize>())
                            .and_then(|bytes| n.checked_add(bytes))
                    })
            })
            .ok_or("New radial byte overflow")?;
        let _radial_charge = self
            .budget()
            .reserve(Kind::Preparation, radial_bytes)
            .map_err(|e| format!("New radial preparation: {e}"))?;
        let mut prepared_new = Vec::new();
        prepared_new
            .try_reserve_exact(edge_total - t.base_edges)
            .map_err(|_| "New radial records refused")?;
        for eid in t.base_edges..edge_total {
            let ids = radial
                .binary_search_by_key(&eid, |x| x.0)
                .ok()
                .map(|i| &radial[i].1[..])
                .unwrap_or(&[]);
            let mut copy = Vec::new();
            copy.try_reserve_exact(ids.len())
                .map_err(|_| "New radial reservation refused")?;
            copy.extend_from_slice(ids);
            prepared_new.push(copy);
        }
        t.edge.force_command = true;
        let edge = self
            .owner
            .prepare(t.edge)?
            .ok_or("Face/edge publication mismatch")?;
        Ok(Some(PreparedFaces {
            edge,
            undo: &mut self.undo,
            redo: &mut self.redo,
            faces,
            prepared_new,
            radial,
            command: Command {
                base_faces: t.base_faces,
                base_edges: t.base_edges,
                faces: retained,
                face_count,
                corners: t.corners,
                incidence: vec![],
                _charge: charge,
            },
            _preparation: prep,
            _radial_charge,
        }))
    }
    pub fn undo(&mut self) -> Result<bool, String> {
        let Some(command) = self.undo.last() else {
            return Ok(false);
        };
        self.redo
            .try_reserve(1)
            .map_err(|_| "Face redo history refused")?;
        if self.mesh().faces.len()
            != command
                .base_faces
                .checked_add(command.face_count)
                .ok_or("Face undo extent overflow")?
            || !command.faces.is_empty()
            || command.faces.capacity() < command.face_count
        {
            return Err("Face undo ownership mismatch".into());
        }
        // The command already owns empty slots; no face/attribute clone or allocation
        // happens after the underlying owner completes its fallible preparation.
        if !self.owner.undo()? {
            return Err("Face/edge undo mismatch".into());
        }
        let mut command = self.undo.pop().unwrap();
        let (faces, edges) = self.owner.face_storage();
        command.faces.extend(faces.drain(command.base_faces..));
        for (eid, _) in &command.incidence {
            if *eid < command.base_edges {
                edges[*eid].faces.retain(|&fid| fid < command.base_faces);
            }
        }
        self.redo.push(command);
        Ok(true)
    }
    pub fn redo(&mut self) -> Result<bool, String> {
        let Some(command) = self.redo.last() else {
            return Ok(false);
        };
        if command.faces.len() != command.face_count {
            return Err("Face redo ownership mismatch".into());
        }
        let prep = self.budget().reserve(
            Kind::Preparation,
            face_bound(command.corners, command.face_count)?,
        )?;
        let mut radial = Vec::new();
        radial
            .try_reserve_exact(command.incidence.len())
            .map_err(|_| "Redo radial refused")?;
        for (eid, ids) in &command.incidence {
            let mut c = Vec::new();
            c.try_reserve_exact(ids.len())
                .map_err(|_| "Redo radial refused")?;
            c.extend_from_slice(ids);
            radial.push((*eid, c));
        }
        self.undo
            .try_reserve(1)
            .map_err(|_| "Face undo history refused")?;
        {
            let (faces, edges) = self.owner.face_storage();
            faces
                .try_reserve(command.face_count)
                .map_err(|_| "Redo face reservation refused")?;
            for (eid, ids) in &radial {
                if *eid < command.base_edges {
                    edges[*eid]
                        .faces
                        .try_reserve(ids.len())
                        .map_err(|_| "Redo existing radial refused")?;
                }
            }
        }
        if !self.owner.redo()? {
            return Err("Face/edge redo mismatch".into());
        }
        let mut command = self.redo.pop().unwrap();
        {
            let (faces, edges) = self.owner.face_storage();
            for (eid, ids) in radial {
                if eid < command.base_edges {
                    edges[eid].faces.extend(ids);
                } else {
                    edges[eid].faces = ids;
                }
            }
            faces.append(&mut command.faces);
        }
        self.undo.push(command);
        drop(prep);
        Ok(true)
    }
}
impl FaceTransaction {
    pub(crate) fn staged_face(&self, id: usize) -> Result<&MeshFace, String> {
        self.drafts
            .get(
                id.checked_sub(self.base_faces)
                    .ok_or("Face is not staged")?,
            )
            .map(|d| &d.face)
            .ok_or("Missing staged face".into())
    }
    pub(crate) fn added_positions(&self) -> &[DVec3] {
        &self.edge.journal.added_vertices
    }

    // Immutable graph-adapter view of already-staged native identities.
    pub(crate) fn lookup_position(&self,p:glam::DVec3)->Result<Option<usize>,String>{self.edge.journal.lookup_position(p)}
    // No endpoint coordinate lookup or Mesh clone is needed to build actual VM Edges.
    pub(crate) fn staged_face_edges(&self, id: usize) -> Result<&[usize], String> {
        self.drafts.get(id.checked_sub(self.base_faces).ok_or("Face is not staged")?)
            .map(|draft| draft.edges.as_slice()).ok_or("Missing staged face".into())
    }
    pub(crate) fn added_edge_endpoints(&self) -> &[(usize, usize)] {
        &self.edge.journal.added_edges
    }
    pub(crate) fn base_edge_extent(&self) -> usize { self.base_edges }

    /// Takes an already-created native JSON map. Arbitrary VM objects/keys remain
    /// a separate required adapter; this API never recursively clones Value.
    pub fn set_face_attributes(
        &mut self,
        id: usize,
        attrs: HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        let result = (|| {
            if self.failed {
                return Err("Face transaction previously refused".into());
            }
            if let Some(view) = &self.edge.journal.registry {
                view.check()?;
            }
            let offset = id
                .checked_sub(self.base_faces)
                .ok_or("Face attributes require a staged face")?;
            let draft = self
                .drafts
                .get(offset)
                .ok_or("Face attributes require a staged face")?;
            let old = attribute_payload(&draft.face.attrs)?;
            let new = attribute_payload(&attrs)?;
            let bytes = self
                .attribute_bytes
                .checked_sub(old)
                .and_then(|n| n.checked_add(new))
                .ok_or("Face attribute byte overflow")?;
            self.attribute_charge.resize(bytes)?;
            self.drafts[offset].face.attrs = attrs;
            self.attribute_bytes = bytes;
            Ok(())
        })();
        if result.is_err() {
            self.failed = true;
        }
        result
    }
    pub fn abort(&mut self) {
        self.failed = true;
    }
    pub fn add_face(&mut self, outer: &[DVec3], holes: &[Vec<DVec3>]) -> Result<usize, String> {
        let result = self.stage_face(outer, holes);
        if result.is_err() {
            self.failed = true;
        }
        result
    }
    fn stage_face(&mut self, outer: &[DVec3], holes: &[Vec<DVec3>]) -> Result<usize, String> {
        if self.failed {
            return Err("Face transaction previously refused".into());
        }
        let count = outer
            .len()
            .checked_add(
                holes
                    .iter()
                    .try_fold(0usize, |n, h| n.checked_add(h.len()))
                    .ok_or("Face corner overflow")?,
            )
            .ok_or("Face corner overflow")?;
        let next = self
            .corners
            .checked_add(count)
            .ok_or("Face corner overflow")?;
        self.charge.resize(face_bound(
            next,
            self.drafts
                .len()
                .checked_add(1)
                .ok_or("Face extent overflow")?,
        )?)?;
        let filtered = if holes.len() > 1 {
            crate::source_holes::maximal(holes)?.0
        } else {
            let mut all = Vec::new();
            all.try_reserve_exact(holes.len())
                .map_err(|_| "Hole filter index refused")?;
            all.extend(0..holes.len());
            all
        };
        let mut rings = Vec::new();
        rings
            .try_reserve_exact(1 + filtered.len())
            .map_err(|_| "Face rings refused")?;
        for ring in std::iter::once(outer).chain(filtered.iter().map(|&i| &holes[i][..])) {
            let mut ids = Vec::new();
            ids.try_reserve_exact(ring.len())
                .map_err(|_| "Face vertex ring refused")?;
            for &p in ring {
                ids.push(self.edge.journal.stage_vertex(p)?);
            }
            rings.push(ids);
        }
        let mut boundary = Vec::new();
        boundary
            .try_reserve_exact(count)
            .map_err(|_| "Face boundary refused")?;
        for ring in &rings {
            for i in 0..ring.len() {
                boundary.push(
                    self.edge
                        .journal
                        .link_vertices(ring[i], ring[(i + 1) % ring.len()])?
                        .0,
                );
            }
        }
        let outer = rings.remove(0);
        let mut face = MeshFace::new(outer);
        face.holes = rings;
        self.drafts
            .try_reserve(1)
            .map_err(|_| "Face staging refused")?;
        let id = self
            .base_faces
            .checked_add(self.drafts.len())
            .ok_or("Face extent overflow")?;
        self.drafts.push(Draft {
            face,
            edges: boundary,
        });
        self.corners = next;
        Ok(id)
    }
    pub fn extrude_geometry(
        &mut self,
        outline: &[[f64; 2]],
        z0: f64,
        z1: f64,
    ) -> Result<(), String> {
        let result = (|| {
            let helper_bytes = outline
                .len()
                .checked_mul(3 * std::mem::size_of::<DVec3>())
                .ok_or("Extrude helper extent overflow")?;
            let _prep = self.budget.reserve(Kind::Preparation, helper_bytes)?;
            let mut lo = Vec::new();
            let mut hi = Vec::new();
            let mut reverse = Vec::new();
            lo.try_reserve_exact(outline.len())
                .map_err(|_| "Extrude lower ring refused")?;
            hi.try_reserve_exact(outline.len())
                .map_err(|_| "Extrude upper ring refused")?;
            reverse
                .try_reserve_exact(outline.len())
                .map_err(|_| "Extrude reversed ring refused")?;
            for p in outline {
                lo.push(DVec3::new(p[0], p[1], z0).as_vec3().as_dvec3());
                hi.push(DVec3::new(p[0], p[1], z1).as_vec3().as_dvec3());
            }
            for i in 0..lo.len() {
                let j = (i + 1) % lo.len();
                self.add_face(&[lo[i], lo[j], hi[j], hi[i]], &[])?;
            }
            reverse.extend(lo.into_iter().rev());
            self.add_face(&reverse, &[])?;
            self.add_face(&hi, &[])?;
            Ok(())
        })();
        if result.is_err() {
            self.failed = true;
        }
        result
    }
}
fn face_bound(corners: usize, faces: usize) -> Result<usize, String> {
    corners
        .checked_mul(2048)
        .and_then(|n| faces.checked_mul(512).and_then(|f| n.checked_add(f)))
        .ok_or_else(|| "Face payload overflow".into())
}
// Counts logical retained payload/capacity. Allocator headers, BTree node slack,
// preexisting caller allocations and VM native memory are explicitly excluded.
fn attribute_payload(attrs: &HashMap<String, serde_json::Value>) -> Result<usize, String> {
    let mut n = attrs
        .capacity()
        .checked_mul(std::mem::size_of::<(String, serde_json::Value)>())
        .ok_or("Attribute map byte overflow")?;
    for (k, v) in attrs {
        let bytes = value_payload(v, 0)?;
        n = n
            .checked_add(k.capacity())
            .and_then(|n| n.checked_add(bytes))
            .ok_or("Attribute payload refused")?;
    }
    Ok(n)
}
fn value_payload(v: &serde_json::Value, depth: usize) -> Result<usize, String> {
    if depth > 64 {
        return Err("Native attribute nesting exceeds 64".into());
    }
    let mut n = 0usize;
    match v {
        serde_json::Value::String(s) => n = s.capacity(),
        serde_json::Value::Array(a) => {
            n = a
                .capacity()
                .checked_mul(std::mem::size_of::<serde_json::Value>())
                .ok_or("Attribute array byte overflow")?;
            for v in a {
                n = n
                    .checked_add(value_payload(v, depth + 1)?)
                    .ok_or("Attribute byte overflow")?;
            }
        }
        serde_json::Value::Object(o) => {
            n = o
                .len()
                .checked_mul(std::mem::size_of::<(String, serde_json::Value)>())
                .ok_or("Attribute object byte overflow")?;
            for (k, v) in o {
                let bytes = value_payload(v, depth + 1)?;
                n = n
                    .checked_add(k.capacity())
                    .and_then(|n| n.checked_add(bytes))
                    .ok_or("Attribute object payload refused")?;
            }
        }
        _ => {}
    }
    Ok(n)
}
fn face_payload(faces: &Vec<MeshFace>, radial: &Vec<(usize, Vec<usize>)>) -> Result<usize, String> {
    let mut bytes = std::mem::size_of::<Command>()
        .checked_add(
            faces
                .capacity()
                .checked_mul(std::mem::size_of::<MeshFace>())
                .ok_or("Face byte overflow")?,
        )
        .ok_or("Face byte overflow")?;
    for f in faces {
        bytes = bytes
            .checked_add(attribute_payload(&f.attrs)?)
            .ok_or("Face attribute history overflow")?;
        let loops = f
            .vertices
            .capacity()
            .checked_mul(std::mem::size_of::<usize>())
            .and_then(|n| {
                f.holes
                    .capacity()
                    .checked_mul(std::mem::size_of::<Vec<usize>>())
                    .and_then(|h| n.checked_add(h))
            })
            .ok_or("Face byte overflow")?;
        bytes = bytes.checked_add(loops).ok_or("Face byte overflow")?;
        for h in &f.holes {
            bytes = bytes
                .checked_add(
                    h.capacity()
                        .checked_mul(std::mem::size_of::<usize>())
                        .ok_or("Face byte overflow")?,
                )
                .ok_or("Face byte overflow")?;
        }
    }
    bytes = bytes
        .checked_add(radial.capacity() * std::mem::size_of::<(usize, Vec<usize>)>())
        .ok_or("Radial byte overflow")?;
    for (_, ids) in radial {
        bytes = bytes
            .checked_add(
                ids.capacity()
                    .checked_mul(std::mem::size_of::<usize>())
                    .ok_or("Radial byte overflow")?,
            )
            .ok_or("Radial byte overflow")?;
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn triangle(t: &mut FaceTransaction) {
        t.add_face(&[DVec3::ZERO, DVec3::X, DVec3::Y], &[]).unwrap();
    }
    #[test]
    fn face_only_command_stales_held_view_and_undo_redo_preserves_radial() {
        let mut o = FaceOwner::new(Mesh::new()).unwrap();
        let mut t = o.begin().unwrap();
        triangle(&mut t);
        o.commit(t).unwrap();
        let mut stale = o.begin().unwrap();
        let mut t = o.begin().unwrap();
        triangle(&mut t);
        o.commit(t).unwrap();
        assert_eq!(o.mesh().vertices.len(), 3);
        assert_eq!(o.mesh().edges.len(), 3);
        assert_eq!(o.mesh().faces.len(), 2);
        assert!(o.mesh().edges.iter().all(|e| e.faces == vec![0, 1]));
        assert!(stale
            .add_face(&[DVec3::ZERO, DVec3::X, DVec3::Y], &[])
            .is_err());
        assert!(o.commit(stale).is_err());
        let saved = o.mesh().clone();
        o.undo().unwrap();
        assert!(o.mesh().edges.iter().all(|e| e.faces == vec![0]));
        o.redo().unwrap();
        assert_eq!(o.mesh(), &saved);
        assert_eq!(o.history_counts(), (2, 0));
    }
    #[test]
    fn two_faces_share_new_edge_in_one_transaction_and_branch_atomically() {
        let mut o = FaceOwner::new(Mesh::new()).unwrap();
        let mut t = o.begin().unwrap();
        triangle(&mut t);
        t.add_face(&[DVec3::X, DVec3::ZERO, -DVec3::Y], &[])
            .unwrap();
        o.commit(t).unwrap();
        assert_eq!(o.mesh().edges[0].faces, vec![0, 1]);
        assert_eq!(o.history_counts(), (1, 0));
        let saved = o.mesh().clone();
        o.undo().unwrap();
        assert!(o.mesh().faces.is_empty());
        o.redo().unwrap();
        assert_eq!(o.mesh(), &saved);
        o.undo().unwrap();
        let mut t = o.begin().unwrap();
        triangle(&mut t);
        o.commit(t).unwrap();
        assert_eq!(o.history_counts(), (1, 0));
        assert!(!o.redo().unwrap());
    }
    #[test]
    fn face_preparation_refusal_preserves_redo_and_earlier_transaction() {
        let mut o = FaceOwner::new(Mesh::new()).unwrap();
        let mut t = o.begin().unwrap();
        triangle(&mut t);
        o.commit(t).unwrap();
        o.undo().unwrap();
        let mut earlier = o.begin().unwrap();
        let mut candidate = o.begin().unwrap();
        triangle(&mut candidate);
        let saved = o.mesh().clone();
        let budget = o.budget();
        let before = budget.snapshot();
        budget.set_limit(Some(before.total()));
        assert!(o.commit(candidate).is_err());
        assert_eq!(o.mesh(), &saved);
        assert_eq!(o.history_counts(), (0, 1));
        budget.set_limit(None);
        triangle(&mut earlier);
        o.commit(earlier).unwrap();
        assert_eq!(o.history_counts(), (1, 0));
    }
    #[test]
    fn maximal_hole_failure_discards_prior_face_drafts_and_preserves_redo() {
        let mut o = FaceOwner::new(Mesh::new()).unwrap();
        let mut t = o.begin().unwrap();
        triangle(&mut t);
        o.commit(t).unwrap();
        o.undo().unwrap();
        let mut earlier = o.begin().unwrap();
        let mut t = o.begin().unwrap();
        triangle(&mut t);
        assert!(t
            .add_face(
                &[DVec3::ZERO, DVec3::X, DVec3::Y],
                &[vec![], vec![DVec3::ZERO, DVec3::X, DVec3::Y]]
            )
            .unwrap_err()
            .contains("requires three"));
        assert!(o.commit(t).is_err());
        assert_eq!(o.history_counts(), (0, 1));
        assert!(o.mesh().vertices.is_empty());
        triangle(&mut earlier);
        o.commit(earlier).unwrap();
        assert_eq!(o.history_counts(), (1, 0));
    }
    #[test]
    fn degenerate_face_membership_once_and_free_edge_refusal_remains_source_owned() {
        let mut o = FaceOwner::new(Mesh::new()).unwrap();
        let mut t = o.begin().unwrap();
        t.add_face(&[DVec3::ZERO], &[]).unwrap();
        o.commit(t).unwrap();
        assert_eq!(o.mesh().vertices[0].edges, vec![0]);
        assert_eq!(o.mesh().edges[0].faces, vec![0]);
        let mut reloaded = FaceOwner::new(o.mesh().clone()).unwrap();
        let mut t = reloaded.begin().unwrap();
        t.add_face(&[DVec3::ZERO, DVec3::X], &[]).unwrap();
        reloaded.commit(t).unwrap();
        assert_eq!(reloaded.mesh().edges[1].faces, vec![1, 1]);
        let saved = reloaded.mesh().clone();
        reloaded.undo().unwrap();
        assert_eq!(reloaded.mesh().vertices[0].edges, vec![0]);
        reloaded.redo().unwrap();
        assert_eq!(reloaded.mesh(), &saved);
        let mut edge = reloaded.owner.begin();
        assert!(edge
            .add_edge(DVec3::ZERO, DVec3::ZERO)
            .unwrap_err()
            .contains("degenerate edge"));
        assert!(!reloaded.owner.commit(edge).unwrap());
    }
    #[test]
    fn caught_face_refusal_never_publishes_prior_drafts() {
        let mut o = FaceOwner::new(Mesh::new()).unwrap();
        let budget = o.budget();
        let mut t = o.begin().unwrap();
        triangle(&mut t);
        assert!(t
            .add_face(&[DVec3::ZERO, DVec3::NAN, DVec3::X], &[])
            .is_err());
        assert!(o.commit(t).is_err());
        assert!(o.mesh().faces.is_empty());
        assert!(o.mesh().vertices.is_empty());
        assert_eq!(o.history_counts(), (0, 0));
        assert_eq!(budget.snapshot().total(), 0);
    }
    #[test]
    fn existing_edge_metadata_survives_face_radial_append_and_history() {
        let mut m = Mesh::new();
        let a = m.add_vertex(DVec3::ZERO);
        let b = m.add_vertex(DVec3::X);
        let eid = m.add_edge(a, b).unwrap();
        m.edges[eid].soft = true;
        m.edges[eid].hidden = true;
        m.edges[eid].layer = Some("Authored".into());
        m.edges[eid].attrs.insert(
            "opaque_vendor".into(),
            serde_json::json!({"nested":[1,"two"]}),
        );
        let old = m.edges[eid].clone();
        let mut o = FaceOwner::new(m).unwrap();
        let mut t = o.begin().unwrap();
        triangle(&mut t);
        o.commit(t).unwrap();
        let painted = o.mesh().clone();
        o.undo().unwrap();
        assert_eq!(o.mesh().edges[eid], old);
        o.redo().unwrap();
        assert_eq!(o.mesh(), &painted);
    }
    #[test]
    fn new_radial_byte_refusal_precedes_edge_publication() {
        let mut o = FaceOwner::new(Mesh::new()).unwrap();
        let mut t = o.begin().unwrap();
        triangle(&mut t);
        let budget = o.budget();
        let baseline = budget.snapshot();
        let mut face = MeshFace::new(vec![0, 1, 2]);
        face.holes.try_reserve_exact(1).unwrap();
        let radial = (0..3)
            .map(|id| {
                let mut ids = Vec::with_capacity(4);
                ids.push(0);
                (id, ids)
            })
            .collect();
        let history = face_payload(&vec![face], &radial).unwrap() + std::mem::size_of::<MeshFace>();
        budget.set_limit(Some(baseline.total() + face_bound(3, 1).unwrap() + history));
        let error = o.commit(t).unwrap_err();
        assert!(error.contains("New radial preparation"), "{error}");
        assert_eq!(o.history_counts(), (0, 0));
        assert!(o.mesh().vertices.is_empty());
        assert!(o.mesh().edges.is_empty());
        assert!(o.mesh().faces.is_empty());
        assert_eq!(budget.snapshot().total(), 0);
        budget.set_limit(None);
        let mut t = o.begin().unwrap();
        triangle(&mut t);
        assert!(o.commit(t).unwrap());
    }
    fn attrs() -> HashMap<String, serde_json::Value> {
        let mut a = HashMap::new();
        a.insert("color".into(), serde_json::json!([0.1, 0.2, 0.3, 0.4]));
        a.insert("layer".into(), serde_json::json!("Authored"));
        a.insert("opacity".into(), serde_json::json!(0.75));
        a.insert("vendor".into(),serde_json::json!({"path":"unchanged.png","nested":[null,true,17,"payload".repeat(8192)]}));
        a
    }
    fn payload_pointer(f: &MeshFace) -> *const u8 {
        f.attrs["vendor"]["nested"][3].as_str().unwrap().as_ptr()
    }
    #[test]
    fn owned_attributes_move_unchanged_and_charge_once_across_history() {
        let mut o = FaceOwner::new(Mesh::new()).unwrap();
        let mut t = o.begin().unwrap();
        let id = t.add_face(&[DVec3::ZERO, DVec3::X, DVec3::Y], &[]).unwrap();
        t.set_face_attributes(id, attrs()).unwrap();
        let ptr = payload_pointer(&t.drafts[0].face);
        let bytes = t.attribute_bytes;
        assert!(bytes > 50000);
        o.commit(t).unwrap();
        assert_eq!(ptr, payload_pointer(&o.mesh().faces[0]));
        let charged = o.budget().snapshot().history;
        let expected = face_payload(&o.mesh().faces, &o.undo[0].incidence).unwrap()
            - (o.mesh().faces.capacity() - o.undo[0].face_count) * std::mem::size_of::<MeshFace>()
            + o.undo[0].faces.capacity() * std::mem::size_of::<MeshFace>();
        assert_eq!(o.undo[0]._charge.bytes(), expected);
        assert!(charged >= expected);
        let mut held = o.begin().unwrap();
        triangle(&mut held);
        o.undo().unwrap();
        assert_eq!(ptr, payload_pointer(&o.redo[0].faces[0]));
        assert_eq!(o.budget().snapshot().history, charged);
        assert!(held
            .set_face_attributes(0, attrs())
            .unwrap_err()
            .contains("Stale"));
        o.redo().unwrap();
        assert_eq!(ptr, payload_pointer(&o.mesh().faces[0]));
        assert_eq!(o.budget().snapshot().history, charged);
        assert_eq!(o.mesh().faces[0].attrs["layer"], "Authored");
    }
    #[test]
    fn attribute_refusal_preserves_redo_and_earlier_transaction() {
        let mut o = FaceOwner::new(Mesh::new()).unwrap();
        let mut t = o.begin().unwrap();
        triangle(&mut t);
        o.commit(t).unwrap();
        o.undo().unwrap();
        let mut earlier = o.begin().unwrap();
        let mut t = o.begin().unwrap();
        triangle(&mut t);
        let budget = o.budget();
        let before = budget.snapshot();
        budget.set_limit(Some(before.total()));
        assert!(t
            .set_face_attributes(0, attrs())
            .unwrap_err()
            .contains("admission"));
        assert!(o.commit(t).is_err());
        assert_eq!(o.history_counts(), (0, 1));
        assert!(o.mesh().faces.is_empty());
        budget.set_limit(None);
        triangle(&mut earlier);
        o.commit(earlier).unwrap();
        assert_eq!(o.history_counts(), (1, 0));
    }
    #[test]
    fn refused_history_preserves_parked_attribute_pointer_and_branch_releases() {
        let mut o = FaceOwner::new(Mesh::new()).unwrap();
        let budget = o.budget();
        let mut t = o.begin().unwrap();
        triangle(&mut t);
        t.set_face_attributes(0, attrs()).unwrap();
        o.commit(t).unwrap();
        let ptr = payload_pointer(&o.mesh().faces[0]);
        let before = budget.snapshot();
        budget.set_limit(Some(before.total()));
        assert!(o.undo().is_err());
        assert_eq!(ptr, payload_pointer(&o.mesh().faces[0]));
        assert_eq!(o.history_counts(), (1, 0));
        assert_eq!(budget.snapshot().total(), before.total());
        budget.set_limit(None);
        o.undo().unwrap();
        let before = budget.snapshot();
        budget.set_limit(Some(before.total()));
        assert!(o.redo().is_err());
        assert_eq!(ptr, payload_pointer(&o.redo[0].faces[0]));
        assert_eq!(o.history_counts(), (0, 1));
        assert_eq!(budget.snapshot().total(), before.total());
        budget.set_limit(None);
        let old_history = budget.snapshot().history;
        let mut t = o.begin().unwrap();
        triangle(&mut t);
        o.commit(t).unwrap();
        assert_eq!(o.history_counts(), (1, 0));
        assert!(budget.snapshot().history + 50000 < old_history);
        drop(o);
        assert_eq!(budget.snapshot().total(), 0);
    }
    #[test]
    fn unsupported_attribute_depth_discards_whole_transaction() {
        let mut o = FaceOwner::new(Mesh::new()).unwrap();
        let mut t = o.begin().unwrap();
        triangle(&mut t);
        let mut v = serde_json::Value::Null;
        for _ in 0..66 {
            v = serde_json::Value::Array(vec![v]);
        }
        let mut a = HashMap::new();
        a.insert("deep".into(), v);
        assert!(t.set_face_attributes(0, a).is_err());
        assert!(o.commit(t).is_err());
        assert!(o.mesh().faces.is_empty());
        assert_eq!(o.history_counts(), (0, 0));
    }
}

impl FaceOwner {
    pub fn prepare_fields(
        &mut self,
        positions: Vec<DVec3>,
        loops: Vec<(Vec<usize>, Vec<Vec<usize>>)>,
    ) -> Result<crate::owner::PreparedFields<'_>, String> {
        self.owner.prepare_fields(positions, loops)
    }
}

impl FaceOwner {
    pub(crate) fn state_token(&self) -> uuid::Uuid {
        self.owner.state_token()
    }
    pub(crate) fn prepare_fields_as(
        &mut self,
        positions: Vec<DVec3>,
        loops: Vec<(Vec<usize>, Vec<Vec<usize>>)>,
        after: uuid::Uuid,
    ) -> Result<crate::owner::PreparedFields<'_>, String> {
        self.owner.prepare_fields_as(positions, loops, after)
    }
}

impl FaceOwner {
    pub fn prepare_membership_as(
        &mut self,
        vertices: &[usize],
        faces: &[usize],
        positions: Vec<DVec3>,
        loops: Vec<(Vec<usize>, Vec<Vec<usize>>)>,
        after: uuid::Uuid,
    ) -> Result<crate::owner::PreparedMembership<'_>, String> {
        self.owner
            .prepare_membership_as(vertices, faces, positions, loops, after)
    }
}

impl FaceOwner {
 pub fn prepare_graph_order_as(&mut self,vertices:&[usize],edges:&[usize],faces:&[usize],positions:Vec<DVec3>,loops:Vec<(Vec<usize>,Vec<Vec<usize>>)>,after:uuid::Uuid)->Result<crate::owner::PreparedMembership<'_>,String>{
  self.owner.prepare_graph_order_as(vertices,edges,faces,positions,loops,after)
 }
}

impl FaceOwner {
 pub fn prepare_identity_replacement_as(&mut self,mesh:Mesh,keys:Vec<(crate::journal::SourceKey,usize)>,state:uuid::Uuid)->Result<crate::owner::PreparedPackedReplacement<'_>,String>{self.owner.prepare_identity_replacement_as(mesh,keys,state)}
 pub fn prepare_packed_replacement_as(&mut self,mesh:Mesh,state:uuid::Uuid)->Result<crate::owner::PreparedPackedReplacement<'_>,String>{
  self.owner.prepare_packed_replacement_as(mesh,state)
 }
}

impl FaceOwner {
 pub fn prepare_edge_replacement_as(&mut self,mesh:Mesh,state:uuid::Uuid)->Result<crate::owner::PreparedPackedReplacement<'_>,String>{self.owner.prepare_edge_replacement_as(mesh,state)}
}
