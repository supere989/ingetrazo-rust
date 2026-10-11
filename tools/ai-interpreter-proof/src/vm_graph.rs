//! Source-owned public Edge/incidence snapshots; endpoints deliberately excluded.
use super::editable_mesh::{Edge, Mesh, Vertex};
use crate::owner_budget::{Kind, Lease, OwnerBudget};
use rustpython_vm::{
    builtins::{PyList, PyListRef, PySet},
    types::AsSequence,
    AsObject, PyObjectRef, PyPayload, PyRef, PyResult, VirtualMachine,
};
struct EdgeFields {
    edge: PyRef<Edge>,
    faces: PyListRef,
    soft: PyObjectRef,
    hidden: PyObjectRef,
    layer: PyObjectRef,
    curve: PyObjectRef,
}
pub(super) struct GraphState {
    members: Vec<PyObjectRef>,
    incidence: Vec<(PyRef<Vertex>, PyObjectRef)>,
    fields: Vec<EdgeFields>,
    charge: Option<Lease>,
}
fn list_copy(list: &PyListRef, vm: &VirtualMachine) -> Result<PyListRef, String> {
    let values = list.borrow_vec();
    let mut copy = Vec::new();
    copy.try_reserve_exact(values.len())
        .map_err(|_| "Edge radial reference reservation refused")?;
    copy.extend(values.iter().cloned());
    Ok(vm.ctx.new_list(copy))
}
fn set_len(value: &PyObjectRef, vm: &VirtualMachine) -> Result<usize, String> {
    value
        .downcast_ref::<PySet>()
        .ok_or("Vertex incidence requires set")?;
    // Static builtin slot: scalar length without members allocation or subclass callbacks.
    (PySet::as_sequence()
        .length
        .ok_or("Builtin set length unavailable")?)(value.sequence_unchecked(), vm)
    .map_err(|_| "Vertex incidence length refused".into())
}
fn set_copy(value: &PyObjectRef, vm: &VirtualMachine) -> Result<PyObjectRef, String> {
    value
        .downcast_ref::<PySet>()
        .ok_or("Vertex incidence requires set")?;
    // Invoke the builtin descriptor directly: preserves stored hashes and never rehashes
    // members or invokes an instance/subclass override while cloning a source set.
    let copy = vm
        .ctx
        .types
        .set_type
        .as_object()
        .get_attr("copy", vm)
        .map_err(|_| "Builtin set copy unavailable")?;
    copy.call((value.clone(),), vm)
        .map_err(|_| "Vertex incidence copy refused".into())
}
fn references(list: &PyListRef) -> Result<Vec<PyObjectRef>, String> {
    let list = list.borrow_vec();
    let mut values = Vec::new();
    values
        .try_reserve_exact(list.len())
        .map_err(|_| "Graph membership references refused")?;
    values.extend(list.iter().cloned());
    Ok(values)
}

impl GraphState {
    // Endpoints are read from genuine live Edge objects: pinned capture_state never
    // snapshots v0/v1. Other fields come from this command-owned source snapshot.
    pub(super) fn packed_edges(&self,vertices:&[PyObjectRef],faces:&[PyObjectRef],budget:&OwnerBudget,vm:&VirtualMachine)->Result<(Vec<ingetrazo_core::mesh::MeshEdge>,Lease),String>{
        use rustpython_vm::builtins::{PyInt,PyStr};
        use num_traits::ToPrimitive;
        let bytes=self.members.len().checked_mul(256).and_then(|n|vertices.len().checked_add(faces.len()).and_then(|m|m.checked_mul(64)).and_then(|m|n.checked_add(m))).ok_or("Packed graph extent overflow")?;
        let mut charge=budget.reserve(Kind::Preparation,bytes)?;
        let mut vertex_ids=std::collections::HashMap::new();vertex_ids.try_reserve(vertices.len()).map_err(|_|"Packed Vertex identity map refused")?;
        for (id,v) in vertices.iter().enumerate(){if vertex_ids.insert(v.get_id(),id).is_some(){return Err("Repeated packed Vertex identity unsupported".into());}}
        let mut face_ids=std::collections::HashMap::new();face_ids.try_reserve(faces.len()).map_err(|_|"Packed Face identity map refused")?;
        for (id,f) in faces.iter().enumerate(){if face_ids.insert(f.get_id(),id).is_some(){return Err("Repeated packed Face identity unsupported".into());}}
        if self.members.len()!=self.fields.len(){return Err("Packed Edge snapshot extent mismatch".into());}
        let mut out=Vec::new();out.try_reserve_exact(self.members.len()).map_err(|_|"Packed Edge slots refused")?;
        let mut ids=std::collections::HashSet::new();ids.try_reserve(self.members.len()).map_err(|_|"Packed Edge identity slots refused")?;
        let boolean=|v:&PyObjectRef|->Result<bool,String>{if v.is(&vm.ctx.true_value){Ok(true)}else if v.is(&vm.ctx.false_value){Ok(false)}else{Err("Packed arbitrary Edge boolean projection pending".into())}};
        for (value,field) in self.members.iter().zip(&self.fields){
            if !ids.insert(value.get_id()){return Err("Repeated packed Edge identity unsupported".into());}
            let edge=value.downcast_ref::<Edge>().ok_or("Packed member requires Edge")?;
            let a=edge.v0.read().clone().ok_or("Cleared packed Edge endpoint")?;let b=edge.v1.read().clone().ok_or("Cleared packed Edge endpoint")?;
            let a=*vertex_ids.get(&a.get_id()).ok_or("Detached packed endpoint projection pending")?;let b=*vertex_ids.get(&b.get_id()).ok_or("Detached packed endpoint projection pending")?;
            let mut native=ingetrazo_core::mesh::MeshEdge::new(a,b);
            native.soft=boolean(&field.soft)?;native.hidden=boolean(&field.hidden)?;
            if !field.layer.is(&vm.ctx.none()) {let text=field.layer.downcast_ref::<PyStr>().ok_or("Arbitrary Edge layer projection pending")?.as_utf8().ok_or("Surrogate Edge layer projection pending")?.as_str();charge.resize(charge.bytes().checked_add(text.len()).ok_or("Packed layer extent overflow")?)?;let mut layer=String::new();layer.try_reserve_exact(text.len()).map_err(|_|"Packed layer allocation refused")?;layer.push_str(text);native.layer=Some(layer);}
            if !field.curve.is(&vm.ctx.none()){native.curve_id=Some(field.curve.downcast_ref::<PyInt>().and_then(|n|n.as_bigint().to_usize()).ok_or("Arbitrary Edge curve projection pending")?);}
            let radial=field.faces.borrow_vec();charge.resize(charge.bytes().checked_add(radial.len().checked_mul(8).ok_or("Packed radial overflow")?).ok_or("Packed radial overflow")?)?;
            native.faces.try_reserve_exact(radial.len()).map_err(|_|"Packed radial allocation refused")?;
            for f in radial.iter(){native.faces.push(*face_ids.get(&f.get_id()).ok_or("Detached radial Face projection pending")?);}
            out.push(native);
        }
        Ok((out,charge))
    }
    pub(super) fn members(&self)->&[PyObjectRef]{&self.members}
    pub(super) fn requires_replacement(&self,mesh:&PyRef<Mesh>,budget:&OwnerBudget)->Result<bool,String>{
        let current=mesh.edges.borrow_vec();if current.len()!=self.members.len(){return Ok(true);}
        let _charge=budget.reserve(Kind::Preparation,self.members.len().checked_mul(64).ok_or("Edge identity extent overflow")?)?;
        let mut previous=std::collections::HashSet::new();previous.try_reserve(self.members.len()).map_err(|_|"Edge identity classification refused")?;
        for value in &self.members{if !previous.insert(value.get_id()){return Err("Repeated prior Edge membership unsupported".into());}}
        for value in current.iter(){if !previous.remove(&value.get_id()){return Ok(true);}}
        // Preserve the existing in-place permutation path rather than repacking it.
        Ok(false)
    }
    pub(super) fn changed_order(&self,mesh:&PyRef<Mesh>,budget:&OwnerBudget)->Result<super::IdentityOrder,String>{super::identity_order(&mesh.edges.borrow_vec(),&self.members,budget)}
    pub(super) fn restore_order(&self,mesh:&PyRef<Mesh>,budget:&OwnerBudget)->Result<super::IdentityOrder,String>{super::identity_order(&self.members,&mesh.edges.borrow_vec(),budget)}

    fn empty() -> Self {
        Self {
            members: vec![],
            incidence: vec![],
            fields: vec![],
            charge: None,
        }
    }
    pub(super) fn capture(
        mesh: &PyRef<Mesh>,
        budget: &OwnerBudget,
        vm: &VirtualMachine,
    ) -> Result<Self, String> {
        let edge_len = mesh.edges.borrow_vec().len();
        let vertex_len = mesh.vertices.borrow_vec().len();
        let mut bytes = edge_len
            .checked_mul(std::mem::size_of::<EdgeFields>() + std::mem::size_of::<PyObjectRef>())
            .and_then(|b| {
                vertex_len
                    .checked_mul(std::mem::size_of::<(PyRef<Vertex>, PyObjectRef)>())
                    .and_then(|v| b.checked_add(v))
            })
            .ok_or("Graph snapshot extent overflow")?;
        let mut charge = budget.reserve(Kind::Preparation, bytes)?;
        let _member_preparation = budget.reserve(
            Kind::Preparation,
            edge_len
                .checked_add(vertex_len)
                .and_then(|n| n.checked_mul(std::mem::size_of::<PyObjectRef>()))
                .ok_or("Graph temporary reference overflow")?,
        )?;
        let edges = references(&mesh.edges)?;
        let vertices = references(&mesh.vertices)?;
        for v in vertices.iter() {
            let v = v
                .downcast_ref::<Vertex>()
                .ok_or("Incidence owner requires Vertex")?;
            let field = v.edges.read().clone().ok_or("Cleared incidence owner")?;
            let count = set_len(&field, vm)?;
            bytes = bytes
                .checked_add(
                    count
                        .checked_mul(std::mem::size_of::<PyObjectRef>())
                        .ok_or("Incidence extent overflow")?,
                )
                .ok_or("Graph extent overflow")?;
        }
        for e in edges.iter() {
            let e = e.downcast_ref::<Edge>().ok_or("Mesh edges requires Edge")?;
            let field = e.faces.read().clone().ok_or("Cleared radial owner")?;
            let list = field
                .downcast_ref::<PyList>()
                .ok_or("Edge faces requires list")?;
            bytes = bytes
                .checked_add(
                    list.borrow_vec()
                        .len()
                        .checked_mul(std::mem::size_of::<PyObjectRef>())
                        .ok_or("Radial extent overflow")?,
                )
                .ok_or("Graph extent overflow")?;
        }
        charge.resize(bytes)?;
        let mut state = Self {
            members: vec![],
            incidence: vec![],
            fields: vec![],
            charge: Some(charge),
        };
        state
            .members
            .try_reserve_exact(edges.len())
            .map_err(|_| "Edge membership reference reservation refused")?;
        state
            .incidence
            .try_reserve_exact(vertices.len())
            .map_err(|_| "Incidence restoration records refused")?;
        state
            .fields
            .try_reserve_exact(edges.len())
            .map_err(|_| "Edge restoration records refused")?;
        state.members.extend(edges.iter().cloned());
        for v in vertices.iter() {
            let v = v
                .clone()
                .downcast::<Vertex>()
                .map_err(|_| "Mesh vertices requires Vertex")?;
            let source = v.edges.read().clone().ok_or("Cleared incidence owner")?;
            let copy = set_copy(&source, vm)?;
            state.incidence.push((v, copy));
        }
        for e in edges.iter() {
            let e = e
                .clone()
                .downcast::<Edge>()
                .map_err(|_| "Mesh edges requires Edge")?;
            let radial = e
                .faces
                .read()
                .clone()
                .ok_or("Cleared radial owner")?
                .downcast::<PyList>()
                .map_err(|_| "Edge faces requires list")?;
            let faces = list_copy(&radial, vm)?;
            let soft = e.soft.read().clone().ok_or("Cleared Edge soft")?;
            let hidden = e.hidden.read().clone().ok_or("Cleared Edge hidden")?;
            let layer = e.layer.read().clone().ok_or("Cleared Edge layer")?;
            let curve = e.curve.read().clone().ok_or("Cleared Edge curve")?;
            state.fields.push(EdgeFields {
                edge: e,
                faces,
                soft,
                hidden,
                layer,
                curve,
            });
        }
        Ok(state)
    }
    pub(super) fn copy(&self, budget: &OwnerBudget, vm: &VirtualMachine) -> Result<Self, String> {
        let mut state = Self {
            members: vec![],
            incidence: vec![],
            fields: vec![],
            charge: Some(budget.reserve(
                Kind::Preparation,
                self.charge.as_ref().map_or(0, |c| c.bytes()),
            )?),
        };
        state
            .members
            .try_reserve_exact(self.members.len())
            .map_err(|_| "History Edge membership refused")?;
        state.members.extend(self.members.iter().cloned());
        state
            .incidence
            .try_reserve_exact(self.incidence.len())
            .map_err(|_| "History incidence records refused")?;
        for (v, s) in &self.incidence {
            state.incidence.push((v.clone(), set_copy(s, vm)?));
        }
        state
            .fields
            .try_reserve_exact(self.fields.len())
            .map_err(|_| "History Edge records refused")?;
        for e in &self.fields {
            state.fields.push(EdgeFields {
                edge: e.edge.clone(),
                faces: list_copy(&e.faces, vm)?,
                soft: e.soft.clone(),
                hidden: e.hidden.clone(),
                layer: e.layer.clone(),
                curve: e.curve.clone(),
            });
        }
        Ok(state)
    }
    pub(super) fn take_for_history(&mut self) -> Self {
        let mut taken = std::mem::replace(self, Self::empty());
        if let Some(c) = &mut taken.charge {
            c.reclassify(Kind::History)
                .expect("Same-owner graph history lease");
        }
        taken
    }
    pub(super) fn validate_membership(
        &self,
        mesh: &PyRef<Mesh>,
        append: bool,
    ) -> Result<(), String> {
        let edges = mesh.edges.borrow_vec();
        if edges.len() < self.members.len()
            || (!append && edges.len() != self.members.len())
            || edges.iter().zip(&self.members).any(|(a, b)| !a.is(b))
        {
            return Err("Edge deletion/replacement/permutation projection pending".into());
        }
        Ok(())
    }
    pub(super) fn identity_live(
        &self,
        mesh: &PyRef<Mesh>,
        budget: &OwnerBudget,
        vm: &VirtualMachine,
    ) -> Result<bool, String> {
        let _members = budget.reserve(
            Kind::Preparation,
            mesh.edges
                .borrow_vec()
                .len()
                .checked_mul(std::mem::size_of::<PyObjectRef>())
                .ok_or("Graph identity members overflow")?,
        )?;
        let edges = references(&mesh.edges)?;
        if edges.len() != self.members.len()
            || edges.iter().zip(&self.members).any(|(a, b)| !a.is(b))
        {
            return Ok(false);
        }
        for (v, before) in &self.incidence {
            let now = v.edges.read().clone().ok_or("Cleared incidence owner")?;
            let n = set_len(&now, vm)?;
            let b = set_len(before, vm)?;
            if n != b {
                return Ok(false);
            }
            let _preparation = budget.reserve(
                Kind::Preparation,
                n.checked_mul(
                    2 * (std::mem::size_of::<usize>() + std::mem::size_of::<PyObjectRef>()),
                )
                .ok_or("Incidence identity extent overflow")?,
            )?;
            let now = now
                .downcast_ref::<PySet>()
                .ok_or("Vertex incidence requires set")?
                .elements();
            let before = before
                .downcast_ref::<PySet>()
                .ok_or("Captured incidence requires set")?
                .elements();
            let mut a = Vec::new();
            let mut b = Vec::new();
            a.try_reserve_exact(now.len())
                .map_err(|_| "Incidence identity preparation refused")?;
            b.try_reserve_exact(before.len())
                .map_err(|_| "Incidence identity preparation refused")?;
            a.extend(now.iter().map(|x| x.get_id()));
            b.extend(before.iter().map(|x| x.get_id()));
            a.sort_unstable();
            b.sort_unstable();
            if a != b {
                return Ok(false);
            }
        }
        for f in &self.fields {
            let now = f
                .edge
                .faces
                .read()
                .clone()
                .ok_or("Cleared radial owner")?
                .downcast::<PyList>()
                .map_err(|_| "Edge faces requires list")?;
            let now = now.borrow_vec();
            let before = f.faces.borrow_vec();
            if now.len() != before.len() || now.iter().zip(before.iter()).any(|(a, b)| !a.is(b)) {
                return Ok(false);
            }
            drop(now);
            drop(before);
            for (slot, before) in [
                (&f.edge.soft, &f.soft),
                (&f.edge.hidden, &f.hidden),
                (&f.edge.layer, &f.layer),
                (&f.edge.curve, &f.curve),
            ] {
                let now = slot.read().clone().ok_or("Cleared Edge field")?;
                if !now.is(before) {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }
    pub(super) fn equivalent_live(
        &self,
        mesh: &PyRef<Mesh>,
        vm: &VirtualMachine,
    ) -> Result<bool, String> {
        let budget = mesh.owner.read().budget();
        let _members = budget.reserve(
            Kind::Preparation,
            mesh.edges
                .borrow_vec()
                .len()
                .checked_mul(std::mem::size_of::<PyObjectRef>())
                .ok_or("Graph equality members overflow")?,
        )?;
        let edges = references(&mesh.edges)?;
        if edges.len() != self.members.len()
            || edges.iter().zip(&self.members).any(|(a, b)| !a.is(b))
        {
            return Ok(false);
        }
        for (v, s) in &self.incidence {
            let now = v.edges.read().clone().ok_or("Cleared incidence owner")?;
            if !vm
                .bool_eq(&now, s)
                .map_err(|_| "Incidence equality failed")?
            {
                return Ok(false);
            }
        }
        for f in &self.fields {
            let values = [
                (&f.edge.faces, f.faces.as_object()),
                (&f.edge.soft, &f.soft),
                (&f.edge.hidden, &f.hidden),
                (&f.edge.layer, &f.layer),
                (&f.edge.curve, &f.curve),
            ];
            for (field, before) in values {
                let now = field.read().clone().ok_or("Cleared Edge field")?;
                if !now.is(before)
                    && !vm
                        .bool_eq(&now, before)
                        .map_err(|_| "Edge field equality failed")?
                {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }
    pub(super) fn members_equivalent(&self, other: &Self) -> bool {
        self.members.len() == other.members.len()
            && self
                .members
                .iter()
                .zip(&other.members)
                .all(|(a, b)| a.is(b))
    }
    pub(super) fn equivalent(&self, other: &Self, vm: &VirtualMachine) -> PyResult<bool> {
        if !self.members_equivalent(other)
            || self.incidence.len() != other.incidence.len()
            || self.fields.len() != other.fields.len()
        {
            return Ok(false);
        }
        for ((a, s), (b, t)) in self.incidence.iter().zip(&other.incidence) {
            if !a.is(b) || !vm.bool_eq(s, t)? {
                return Ok(false);
            }
        }
        // Match source capture_state dictionary family order; do not interleave Edge fields.
        for (a, b) in self.fields.iter().zip(&other.fields) {
            if !a.edge.is(&b.edge) || !vm.bool_eq(a.faces.as_object(), b.faces.as_object())? {
                return Ok(false);
            }
        }
        for family in 0..4 {
            for (a, b) in self.fields.iter().zip(&other.fields) {
                let (x, y) = match family {
                    0 => (&a.soft, &b.soft),
                    1 => (&a.hidden, &b.hidden),
                    2 => (&a.layer, &b.layer),
                    _ => (&a.curve, &b.curve),
                };
                if !x.is(y) && !vm.bool_eq(x, y)? {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }
    pub(super) fn restore(&mut self, mesh: &PyRef<Mesh>) {
        let old = std::mem::replace(
            &mut *mesh.edges.borrow_vec_mut(),
            std::mem::take(&mut self.members),
        );
        drop(old);
        for (v, s) in self.incidence.drain(..) {
            let old = std::mem::replace(&mut *v.edges.write(), Some(s));
            drop(old);
        }
        for f in self.fields.drain(..) {
            for (slot, value) in [
                (&f.edge.faces, f.faces.into()),
                (&f.edge.soft, f.soft),
                (&f.edge.hidden, f.hidden),
                (&f.edge.layer, f.layer),
                (&f.edge.curve, f.curve),
            ] {
                let old = std::mem::replace(&mut *slot.write(), Some(value));
                drop(old);
            }
        }
    }
}
