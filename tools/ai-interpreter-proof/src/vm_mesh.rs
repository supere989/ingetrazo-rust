//! Genuine editable VM Mesh prerequisite, append-only staged mutation.
use crate::{
    face_owner::{FaceOwner, FaceTransaction},
    owner_budget::{Kind, Lease, OwnerBudget},
    session,
};
use rustpython_vm::{
    compiler::Mode, pymodule, scope::Scope, Interpreter, InterpreterBuilder, PyRef,
};
use std::{cell::RefCell, collections::HashMap};
#[path = "vm_graph.rs"]
mod graph;
use graph::GraphState;
// Drop removed arbitrary VM references only after releasing the membership lock.
fn discard_tail(list: &rustpython_vm::builtins::PyListRef, keep: usize) {
    loop {
        let removed = {
            let mut values = list.borrow_vec_mut();
            if values.len() > keep {
                values.pop()
            } else {
                None
            }
        };
        let Some(removed) = removed else { break };
        drop(removed);
    }
}
struct Entry {
    mesh: PyRef<editable_mesh::Mesh>,
    transaction: Option<FaceTransaction>,
    faces: usize,
    vertices: usize,
    published: bool,
    vertices_created: Vec<PyRef<editable_mesh::Vertex>>,
    faces_created: Vec<PyRef<editable_mesh::Face>>,
    edges_created: Vec<PyRef<editable_mesh::Edge>>,
    graph_before: Option<GraphState>,
    handle_charge: Lease,
}
impl Drop for Entry {
    fn drop(&mut self) {
        if !self.published {
            if let Some(mut graph)=self.graph_before.take(){graph.restore(&self.mesh);}
            discard_tail(&self.mesh.faces, self.faces);
            discard_tail(&self.mesh.vertices, self.vertices);
        }
    }
}
thread_local! { static GRAPH_REENTRY_PROBE: std::cell::Cell<u8> = const { std::cell::Cell::new(0) }; }

impl Entry {
    fn validate_created(&self) -> Result<(), String> {
        use rustpython_vm::AsObject;
        let vertices = self.mesh.vertices.borrow_vec();
        let faces = self.mesh.faces.borrow_vec();
        let edges = self.mesh.edges.borrow_vec();
        let transaction = self
            .transaction
            .as_ref()
            .ok_or("Missing staged transaction")?;
        if vertices.len() != self.vertices + self.vertices_created.len()
            || faces.len() != self.faces + self.faces_created.len()
        {
            return Err("Staged membership modification unsupported".into());
        }
        if edges.len() != transaction.base_edge_extent() + self.edges_created.len() {
            return Err("Staged Edge membership modification unsupported".into());
        }
        for (i, edge) in self.edges_created.iter().enumerate() {
            if !edges[transaction.base_edge_extent() + i].is(edge) {
                return Err("Staged Edge identity/order modification unsupported".into());
            }
            let (a,b) = transaction.added_edge_endpoints()[i];
            if !edge.v0.read().as_ref().ok_or("Cleared Edge endpoint")?.is(&vertices[a])
                || !edge.v1.read().as_ref().ok_or("Cleared Edge endpoint")?.is(&vertices[b]) {
                return Err("Staged Edge endpoint modification requires packed projection".into());
            }
        }
        for (i, v) in self.vertices_created.iter().enumerate() {
            let incidence = v
                .edges
                .read()
                .clone()
                .expect("Live Vertex incidence")
                .downcast::<rustpython_vm::builtins::PySet>()
                .map_err(|_| "Staged incidence edit requires Edge graph journal")?;
            // Genuine generated incidence now exists; graph snapshot owns rollback.
            for edge in incidence.elements() {
                if edge.downcast_ref::<editable_mesh::Edge>().is_none() {return Err("Incidence requires Edge identity".into());}
            }
            if !vertices[self.vertices + i].is(v) {
                return Err("Staged Vertex identity/order modification unsupported".into());
            }
            if v.position.read().coordinates()
                != transaction.added_positions()[i].as_vec3().to_array()
            {
                return Err("Staged Vertex position modification unsupported".into());
            }
        }
        for (i, f) in self.faces_created.iter().enumerate() {
            if !faces[self.faces + i].is(f) {
                return Err("Staged Face identity/order modification unsupported".into());
            }
            let native = transaction.staged_face(self.faces + i)?;
            let outer = f.r#loop.read().clone();
            let outer = outer.borrow_vec();
            if outer.len() != native.vertices.len()
                || outer
                    .iter()
                    .zip(&native.vertices)
                    .any(|(v, &id)| !v.is(&vertices[id]))
            {
                return Err("Staged Face loop modification unsupported".into());
            }
            let holes = f.hole_loops.read().clone();
            let holes = holes.borrow_vec();
            if holes.len() != native.holes.len() {
                return Err("Staged Face holes modification unsupported".into());
            }
            for (h, ids) in holes.iter().zip(&native.holes) {
                let h = h
                    .downcast_ref::<rustpython_vm::builtins::PyList>()
                    .ok_or("Staged hole requires list")?;
                let h = h.borrow_vec();
                if h.len() != ids.len() || h.iter().zip(ids).any(|(v, &id)| !v.is(&vertices[id])) {
                    return Err("Staged Face hole modification unsupported".into());
                }
            }
        }
        Ok(())
    }
}
// Cold engineering observations of actual VM payload destruction, not OwnerBudget bytes.
static LIVE_VERTICES: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static LIVE_EDGES: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
pub fn graph_payload_lifetimes() -> (usize, usize) {
    use std::sync::atomic::Ordering;
    (
        LIVE_VERTICES.load(Ordering::Relaxed),
        LIVE_EDGES.load(Ordering::Relaxed),
    )
}
struct Stage {
    entries: HashMap<usize, Entry>,
    budget: OwnerBudget,
    charge: Lease,
    failed: bool,
}
impl Stage {
    fn rollback(&mut self) {
        for entry in self.entries.values_mut() {
            if let Some(mut graph)=entry.graph_before.take(){graph.restore(&entry.mesh);}
            discard_tail(&entry.mesh.faces, entry.faces);
            discard_tail(&entry.mesh.vertices, entry.vertices);
        }
    }
}
thread_local! {static STAGED:RefCell<Option<Stage>>=const{RefCell::new(None)};}
struct Guard<'a>(&'a rustpython_vm::VirtualMachine);
impl Drop for Guard<'_> {
    fn drop(&mut self) {
        self.0.host_budget_abort();
        let stage = STAGED.with(|s| s.borrow_mut().take());
        if let Some(mut stage) = stage {
            stage.rollback();
        }
    }
}
#[pymodule]
pub mod editable_mesh {
    use super::*;
    use crate::vm_groups::native_groups::QVector3D;
    use rustpython_vm::{
        builtins::{PyListRef, PySet, PyType},
        common::lock::PyRwLock,
        function::{OptionalArg, PosArgs},
        pyclass,
        types::Constructor,
        AsObject, Py, PyObjectRef, PyPayload, PyResult, Traverse, VirtualMachine,
    };
    // Cold engineering-only collection under the existing script fuel; never resets poison.
    #[pyfunction]
    fn proof_collect_gc(vm: &VirtualMachine) {
        vm.state.gc.collect_force(2);
    }
    #[pyattr]
    #[pyclass(name = "Vertex", traverse = "manual")]
    #[derive(Debug, PyPayload)]
    pub struct Vertex {
        pub(super) position: PyRwLock<PyRef<QVector3D>>,
        pub(super) edges: PyRwLock<Option<PyObjectRef>>,
    }
    unsafe impl rustpython_vm::object::Traverse for Vertex {
        fn traverse(&self, visit: &mut rustpython_vm::object::TraverseFn<'_>) {
            self.position.traverse(visit);
            self.edges.traverse(visit);
        }
        fn clear(&mut self, out: &mut Vec<PyObjectRef>) {
            // Collector only calls clear for unreachable payloads. Move the owning
            // incidence ref to its deferred drop list; no VM allocation or callback
            // occurs while this field lock is held.
            let edges = { self.edges.write().take() };
            if let Some(edges) = edges {
                out.push(edges);
            }
        }
    }
    impl Vertex {
        fn track(self) -> Self {
            LIVE_VERTICES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            self
        }
    }
    impl Drop for Vertex {
        fn drop(&mut self) {
            LIVE_VERTICES.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
        }
    }
    impl Constructor for Vertex {
        type Args = PyRef<QVector3D>;
        fn py_new(_cls: &Py<PyType>, position: Self::Args, vm: &VirtualMachine) -> PyResult<Self> {
            vm.host_budget_tick()?;
            Ok(Self {
                position: PyRwLock::new(
                    QVector3D::from_coordinates(position.coordinates()).into_ref(&vm.ctx),
                ),
                edges: PyRwLock::new(Some(PySet::default().into_ref(&vm.ctx).into())),
            }
            .track())
        }
    }
    #[pyclass(with(Constructor))]
    impl Vertex {
        #[pygetset]
        fn position(zelf: &Py<Self>) -> PyRef<QVector3D> {
            zelf.position.read().clone()
        }
        #[pygetset(setter)]
        fn set_position(zelf: &Py<Self>, position: PyRef<QVector3D>) {
            let old = std::mem::replace(&mut *zelf.position.write(), position);
            drop(old);
        }
        #[pygetset]
        fn edges(zelf: &Py<Self>) -> PyObjectRef {
            zelf.edges.read().clone().expect("Live Vertex incidence")
        }
        #[pygetset(setter)]
        fn set_edges(zelf: &Py<Self>, edges: PyObjectRef) {
            let old = std::mem::replace(&mut *zelf.edges.write(), Some(edges));
            drop(old);
        }
        #[pymethod]
        fn faces(zelf: &Py<Self>, vm: &VirtualMachine) -> PyResult<PyObjectRef> {
            vm.host_budget_tick()?;
            let edges = zelf.edges.read().clone().expect("Live Vertex incidence");
            let out = PySet::default().into_ref(&vm.ctx);
            for edge in vm.extract_elements_with(&edges, Ok)? {
                let radial = edge.get_attr("faces", vm)?;
                for face in vm.extract_elements_with(&radial, Ok)? {
                    vm.host_budget_tick()?;
                    out.add(face, vm)?;
                }
            }
            Ok(out.into())
        }
    }
    #[pyattr]
    #[pyclass(name = "Edge", traverse = "manual")]
    #[derive(Debug, PyPayload)]
    pub struct Edge {
        pub(super) v0: PyRwLock<Option<PyRef<Vertex>>>,
        pub(super) v1: PyRwLock<Option<PyRef<Vertex>>>,
        pub(super) faces: PyRwLock<Option<PyObjectRef>>,
        pub(super) soft: PyRwLock<Option<PyObjectRef>>,
        pub(super) hidden: PyRwLock<Option<PyObjectRef>>,
        pub(super) layer: PyRwLock<Option<PyObjectRef>>,
        pub(super) curve: PyRwLock<Option<PyObjectRef>>,
    }
    unsafe impl rustpython_vm::object::Traverse for Edge {
        fn traverse(&self, visit: &mut rustpython_vm::object::TraverseFn<'_>) {
            self.v0.traverse(visit);
            self.v1.traverse(visit);
            self.faces.traverse(visit);
            self.soft.traverse(visit);
            self.hidden.traverse(visit);
            self.layer.traverse(visit);
            self.curve.traverse(visit);
        }
        fn clear(&mut self, out: &mut Vec<PyObjectRef>) {
            let v0 = { self.v0.write().take() };
            if let Some(v0) = v0 {
                out.push(v0.into());
            }
            let v1 = { self.v1.write().take() };
            if let Some(v1) = v1 {
                out.push(v1.into());
            }
            for field in [
                &self.faces,
                &self.soft,
                &self.hidden,
                &self.layer,
                &self.curve,
            ] {
                let child = { field.write().take() };
                if let Some(child) = child {
                    out.push(child);
                }
            }
        }
    }
    impl Edge {
        pub(super) fn generated(v0:PyRef<Vertex>,v1:PyRef<Vertex>,vm:&VirtualMachine)->PyRef<Self> {
            Self {v0:PyRwLock::new(Some(v0)),v1:PyRwLock::new(Some(v1)),faces:PyRwLock::new(Some(vm.ctx.new_list(vec![]).into())),soft:PyRwLock::new(Some(vm.ctx.new_bool(false).into())),hidden:PyRwLock::new(Some(vm.ctx.new_bool(false).into())),layer:PyRwLock::new(Some(vm.ctx.none())),curve:PyRwLock::new(Some(vm.ctx.none()))}.track().into_ref(&vm.ctx)
        }
        fn track(self) -> Self {
            LIVE_EDGES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            self
        }
    }
    impl Drop for Edge {
        fn drop(&mut self) {
            LIVE_EDGES.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
        }
    }
    impl Constructor for Edge {
        type Args = (PyRef<Vertex>, PyRef<Vertex>);
        fn py_new(_cls: &Py<PyType>, (v0, v1): Self::Args, vm: &VirtualMachine) -> PyResult<Self> {
            vm.host_budget_tick()?;
            Ok(Self {
                v0: PyRwLock::new(Some(v0)),
                v1: PyRwLock::new(Some(v1)),
                faces: PyRwLock::new(Some(vm.ctx.new_list(vec![]).into())),
                soft: PyRwLock::new(Some(vm.ctx.new_bool(false).into())),
                hidden: PyRwLock::new(Some(vm.ctx.new_bool(false).into())),
                layer: PyRwLock::new(Some(vm.ctx.none())),
                curve: PyRwLock::new(Some(vm.ctx.none())),
            }
            .track())
        }
    }
    #[pyclass(with(Constructor))]
    impl Edge {
        #[pygetset]
        fn v0(zelf: &Py<Self>) -> PyRef<Vertex> {
            zelf.v0.read().clone().expect("Live Edge field")
        }
        #[pygetset(setter)]
        fn set_v0(zelf: &Py<Self>, v: PyRef<Vertex>) {
            let old = std::mem::replace(&mut *zelf.v0.write(), Some(v));
            drop(old);
        }
        #[pygetset]
        fn v1(zelf: &Py<Self>) -> PyRef<Vertex> {
            zelf.v1.read().clone().expect("Live Edge field")
        }
        #[pygetset(setter)]
        fn set_v1(zelf: &Py<Self>, v: PyRef<Vertex>) {
            let old = std::mem::replace(&mut *zelf.v1.write(), Some(v));
            drop(old);
        }
        #[pygetset]
        fn a(zelf: &Py<Self>) -> PyRef<QVector3D> {
            zelf.v0
                .read()
                .as_ref()
                .expect("Live Edge endpoint")
                .position
                .read()
                .clone()
        }
        #[pygetset]
        fn b(zelf: &Py<Self>) -> PyRef<QVector3D> {
            zelf.v1
                .read()
                .as_ref()
                .expect("Live Edge endpoint")
                .position
                .read()
                .clone()
        }
        #[pygetset]
        fn faces(zelf: &Py<Self>) -> PyObjectRef {
            zelf.faces.read().clone().expect("Live Edge field")
        }
        #[pygetset(setter)]
        fn set_faces(zelf: &Py<Self>, v: PyObjectRef) {
            let old = std::mem::replace(&mut *zelf.faces.write(), Some(v));
            drop(old);
        }
        #[pygetset]
        fn soft(zelf: &Py<Self>) -> PyObjectRef {
            zelf.soft.read().clone().expect("Live Edge field")
        }
        #[pygetset(setter)]
        fn set_soft(zelf: &Py<Self>, v: PyObjectRef) {
            let old = std::mem::replace(&mut *zelf.soft.write(), Some(v));
            drop(old);
        }
        #[pygetset]
        fn hidden(zelf: &Py<Self>) -> PyObjectRef {
            zelf.hidden.read().clone().expect("Live Edge field")
        }
        #[pygetset(setter)]
        fn set_hidden(zelf: &Py<Self>, v: PyObjectRef) {
            let old = std::mem::replace(&mut *zelf.hidden.write(), Some(v));
            drop(old);
        }
        #[pygetset]
        fn layer(zelf: &Py<Self>) -> PyObjectRef {
            zelf.layer.read().clone().expect("Live Edge field")
        }
        #[pygetset(setter)]
        fn set_layer(zelf: &Py<Self>, v: PyObjectRef) {
            let old = std::mem::replace(&mut *zelf.layer.write(), Some(v));
            drop(old);
        }
        #[pygetset]
        fn curve(zelf: &Py<Self>) -> PyObjectRef {
            zelf.curve.read().clone().expect("Live Edge field")
        }
        #[pygetset(setter)]
        fn set_curve(zelf: &Py<Self>, v: PyObjectRef) {
            let old = std::mem::replace(&mut *zelf.curve.write(), Some(v));
            drop(old);
        }
        #[pymethod]
        fn other(zelf: &Py<Self>, vertex: PyObjectRef) -> PyRef<Vertex> {
            if zelf
                .v0
                .read()
                .as_ref()
                .expect("Live Edge endpoint")
                .is(&vertex)
            {
                zelf.v1.read().clone().expect("Live Edge field")
            } else {
                zelf.v0.read().clone().expect("Live Edge field")
            }
        }
        #[pymethod]
        fn has(zelf: &Py<Self>, vertex: PyObjectRef) -> bool {
            zelf.v0
                .read()
                .as_ref()
                .expect("Live Edge endpoint")
                .is(&vertex)
                || zelf
                    .v1
                    .read()
                    .as_ref()
                    .expect("Live Edge endpoint")
                    .is(&vertex)
        }
        #[pymethod]
        fn length(zelf: &Py<Self>, vm: &VirtualMachine) -> PyResult<f64> {
            vm.host_budget_tick()?;
            let a = zelf
                .v0
                .read()
                .as_ref()
                .expect("Live Edge endpoint")
                .position
                .read()
                .coordinates();
            let b = zelf
                .v1
                .read()
                .as_ref()
                .expect("Live Edge endpoint")
                .position
                .read()
                .coordinates();
            Ok(QVector3D::magnitude(std::array::from_fn(|i| b[i] - a[i])) as f64)
        }
    }
    #[pyattr]
    #[pyclass(name = "Face", traverse)]
    #[derive(Debug, PyPayload)]
    pub struct Face {
        pub(super) r#loop: PyRwLock<PyListRef>,
        pub(super) hole_loops: PyRwLock<PyListRef>,
        pub(super) attrs: PyRwLock<PyObjectRef>,
    }
    #[pyclass]
    impl Face {
        #[pygetset(name = "loop")]
        fn face_loop(zelf: &Py<Self>) -> PyListRef {
            zelf.r#loop.read().clone()
        }
        #[pygetset]
        fn hole_loops(zelf: &Py<Self>) -> PyListRef {
            zelf.hole_loops.read().clone()
        }
        #[pygetset]
        fn vertices(zelf: &Py<Self>, vm: &VirtualMachine) -> PyResult<PyListRef> {
            vm.host_budget_tick()?;
            let mut positions = Vec::new();
            let list = zelf.r#loop.read().clone();
            let ring = list.borrow_vec();
            positions
                .try_reserve_exact(ring.len())
                .map_err(|_| vm.new_value_error("Face position slots refused"))?;
            for vertex in ring.iter() {
                let vertex = vertex
                    .downcast_ref::<Vertex>()
                    .ok_or_else(|| vm.new_type_error("Face loop requires Vertex"))?;
                positions.push(vertex.position.read().clone().into());
            }
            Ok(vm.ctx.new_list(positions))
        }
        #[pymethod]
        fn normal(zelf: &Py<Self>, vm: &VirtualMachine) -> PyResult<PyRef<QVector3D>> {
            vm.host_budget_tick()?;
            let list = zelf.r#loop.read().clone();
            let ring = list.borrow_vec();
            let mut n = [0f64; 3];
            for i in 0..ring.len() {
                let p = ring[i]
                    .downcast_ref::<Vertex>()
                    .ok_or_else(|| vm.new_type_error("Face loop requires Vertex"))?
                    .position
                    .read()
                    .coordinates()
                    .map(f64::from);
                let q = ring[(i + 1) % ring.len()]
                    .downcast_ref::<Vertex>()
                    .ok_or_else(|| vm.new_type_error("Face loop requires Vertex"))?
                    .position
                    .read()
                    .coordinates()
                    .map(f64::from);
                n[0] += (p[1] - q[1]) * (p[2] + q[2]);
                n[1] += (p[2] - q[2]) * (p[0] + q[0]);
                n[2] += (p[0] - q[0]) * (p[1] + q[1]);
            }
            let n = n.map(|x| x as f32);
            let magnitude = QVector3D::magnitude(n);
            let result = if ring.len() < 3 || (magnitude as f64) < 1e-9 {
                [0., 0., 1.]
            } else {
                n.map(|x| x / magnitude)
            };
            Ok(QVector3D::from_coordinates(result).into_ref(&vm.ctx))
        }
        #[pymethod]
        fn centroid(zelf: &Py<Self>, vm: &VirtualMachine) -> PyResult<PyRef<QVector3D>> {
            vm.host_budget_tick()?;
            let list = zelf.r#loop.read().clone();
            let ring = list.borrow_vec();
            let mut sum = [0f64; 3];
            for v in ring.iter() {
                let p = v
                    .downcast_ref::<Vertex>()
                    .ok_or_else(|| vm.new_type_error("Face loop requires Vertex"))?
                    .position
                    .read()
                    .coordinates();
                for i in 0..3 {
                    sum[i] += p[i] as f64;
                }
            }
            let result = if ring.is_empty() {
                [0.; 3]
            } else {
                sum.map(|x| (x / ring.len() as f64) as f32)
            };
            Ok(QVector3D::from_coordinates(result).into_ref(&vm.ctx))
        }
        #[pygetset]
        fn attrs(zelf: &Py<Self>) -> PyObjectRef {
            zelf.attrs.read().clone()
        }
    }
    #[pyattr]
    #[pyclass(name = "Mesh", traverse)]
    #[derive(PyPayload)]
    pub struct Mesh {
        #[pytraverse(skip)]
        pub(super) owner: PyRwLock<FaceOwner>,
        pub(super) faces: PyListRef,
        pub(super) vertices: PyListRef,
        pub(super) edges: PyListRef,
        #[pytraverse(skip)]
        pub(super) managed: std::cell::Cell<bool>,
    }
    impl std::fmt::Debug for Mesh {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("EditableMesh")
        }
    }
    impl Constructor for Mesh {
        type Args = PosArgs<PyObjectRef>;
        fn py_new(_cls: &Py<PyType>, args: Self::Args, vm: &VirtualMachine) -> PyResult<Self> {
            vm.host_budget_tick()?;
            if !args.into_vec().is_empty() {
                return Err(vm.new_type_error("Mesh takes no arguments"));
            }
            let budget = STAGED
                .with(|s| s.try_borrow().map(|slot| slot.as_ref().map(|s| s.budget.clone())))
                .map_err(|_| {vm.host_budget_abort(); vm.new_value_error("Reentrant Mesh construction refused")})?
                .ok_or_else(|| vm.new_value_error("Mesh requires active transaction"))?;
            let owner = FaceOwner::new_with_budget(ingetrazo_core::mesh::Mesh::new(), budget)
                .map_err(|e| vm.new_value_error(e))?;
            Ok(Self {
                owner: PyRwLock::new(owner),
                faces: vm.ctx.new_list(vec![]),
                vertices: vm.ctx.new_list(vec![]),
                edges: vm.ctx.new_list(vec![]),
                managed: std::cell::Cell::new(false),
            })
        }
    }
    #[pyclass(with(Constructor))]
    impl Mesh {
        #[pygetset]
        fn edges(zelf: &Py<Self>) -> PyListRef { zelf.edges.clone() }
        #[pygetset]
        fn faces(zelf: &Py<Self>) -> PyListRef {
            zelf.faces.clone()
        }
        #[pygetset]
        fn vertices(zelf: &Py<Self>) -> PyListRef {
            zelf.vertices.clone()
        }
        #[pymethod]
        fn add_face(
            zelf: &Py<Self>,
            outer: PyObjectRef,
            holes: OptionalArg<PyObjectRef>,
            vm: &VirtualMachine,
        ) -> PyResult<PyRef<Face>> {
            vm.host_budget_tick()?;
            let extract = |value: PyObjectRef| {
                vm.extract_elements_with(&value, |v| {
                    let v = v
                        .downcast::<QVector3D>()
                        .map_err(|_| vm.new_type_error("Face vertices require QVector3D"))?;
                    let p = v.coordinates();
                    Ok(glam::DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64))
                })
            };
            let outer = extract(outer)?;
            let holes = match holes {
                OptionalArg::Present(v) => vm.extract_elements_with(&v, extract)?,
                OptionalArg::Missing => vec![],
            };
            let result = STAGED.with(|cell| {
                let mut slot = cell.try_borrow_mut().map_err(|_| { vm.host_budget_abort(); vm.new_value_error("Reentrant Mesh mutation refused") })?;
                let stage = slot.as_mut().ok_or_else(|| {
                    vm.new_value_error("Mesh mutation requires active transaction")
                })?;
                if stage.failed {
                    return Err(vm.new_value_error("Transaction previously refused"));
                }
                let operation = (|| {
                    let key = zelf.get_id();
                    if !stage.entries.contains_key(&key) {
                        stage
                            .entries
                            .try_reserve(1)
                            .map_err(|_| vm.new_value_error("Mesh journal slots refused"))?;
                        stage
                            .charge
                            .resize(
                                (stage.entries.len() + 1)
                                    .checked_mul(std::mem::size_of::<Entry>())
                                    .ok_or_else(|| {
                                        vm.new_value_error("Mesh journal extent overflow")
                                    })?,
                            )
                            .map_err(|e| vm.new_value_error(e))?;
                        let transaction = zelf
                            .owner
                            .read()
                            .begin()
                            .map_err(|e| vm.new_value_error(e))?;
                        stage.entries.insert(
                            key,
                            Entry {
                                mesh: zelf.to_owned(),
                                transaction: Some(transaction),
                                faces: zelf.faces.borrow_vec().len(),
                                vertices: zelf.vertices.borrow_vec().len(),
                                published: false,
                                vertices_created: Vec::new(),
                                faces_created: Vec::new(),
                                edges_created: Vec::new(),
                                graph_before: Some(GraphState::capture(&zelf.to_owned(),&stage.budget,vm).map_err(|e|vm.new_value_error(e))?),
                                handle_charge: stage
                                    .budget
                                    .reserve(Kind::Staging, 0)
                                    .map_err(|e| vm.new_value_error(e))?,
                            },
                        );
                    }
                    let entry = stage.entries.get_mut(&key).unwrap();
                    let transaction = entry.transaction.as_mut().unwrap();
                    let id = transaction
                        .add_face(&outer, &holes)
                        .map_err(|e| vm.new_value_error(e))?;
                    let native = transaction
                        .staged_face(id)
                        .map_err(|e| vm.new_value_error(e))?;
                    let base = entry.vertices;
                    let mut vertices = zelf.vertices.borrow_vec_mut();
                    let positions = transaction.added_positions();
                    let new = base + positions.len();
                    let growth = new
                        .checked_sub(vertices.len())
                        .ok_or_else(|| vm.new_value_error("Vertex list ownership mismatch"))?;
                    let handle_slots = positions
                        .len()
                        .checked_add(entry.faces_created.len())
                        .and_then(|n|n.checked_add(transaction.added_edge_endpoints().len()))
                        .and_then(|n| n.checked_add(1))
                        .and_then(|n| {
                            n.checked_mul(std::mem::size_of::<rustpython_vm::PyObjectRef>())
                        })
                        .ok_or_else(|| vm.new_value_error("Staged handle extent overflow"))?;
                    entry
                        .handle_charge
                        .resize(handle_slots)
                        .map_err(|e| vm.new_value_error(e))?;
                    entry.vertices_created.try_reserve(growth).map_err(|_| {
                        vm.new_value_error("Staged Vertex identity reservation refused")
                    })?;
                    let new_edges=transaction.added_edge_endpoints().len().checked_sub(entry.edges_created.len()).ok_or_else(||vm.new_value_error("Staged edge ownership mismatch"))?;
                    entry.edges_created.try_reserve(new_edges).map_err(|_|vm.new_value_error("Staged Edge identity reservation refused"))?;
                    entry.faces_created.try_reserve(1).map_err(|_| {
                        vm.new_value_error("Staged Face identity reservation refused")
                    })?;
                    let capacity = entry
                        .vertices_created
                        .capacity()
                        .checked_add(entry.faces_created.capacity())
                        .and_then(|n|n.checked_add(entry.edges_created.capacity()))
                        .and_then(|n| {
                            n.checked_mul(std::mem::size_of::<rustpython_vm::PyObjectRef>())
                        })
                        .ok_or_else(|| vm.new_value_error("Staged handle capacity overflow"))?;
                    entry
                        .handle_charge
                        .resize(capacity)
                        .map_err(|e| vm.new_value_error(e))?;
                    vertices
                        .try_reserve(growth)
                        .map_err(|_| vm.new_value_error("Vertex list reservation refused"))?;
                    for p in &positions[vertices.len() - base..] {
                        let position =
                            QVector3D::from_coordinates(p.as_vec3().to_array()).into_ref(&vm.ctx);
                        let vertex = Vertex {
                            position: PyRwLock::new(position),
                            edges: PyRwLock::new(Some(PySet::default().into_ref(&vm.ctx).into())),
                        }
                        .track()
                        .into_ref(&vm.ctx);
                        entry.vertices_created.push(vertex.clone());
                        vertices.push(vertex.into());
                    }
                    let build_ring = |ids: &[usize]| -> PyResult<PyListRef> {
                        let mut ring = Vec::new();
                        ring.try_reserve_exact(ids.len())
                            .map_err(|_| vm.new_value_error("Face loop reservation refused"))?;
                        for &id in ids {
                            ring.push(
                                vertices
                                    .get(id)
                                    .ok_or_else(|| {
                                        vm.new_value_error("Face vertex ownership mismatch")
                                    })?
                                    .clone(),
                            );
                        }
                        Ok(vm.ctx.new_list(ring))
                    };
                    let r#loop = build_ring(&native.vertices)?;
                    let mut hole_lists = Vec::new();
                    hole_lists
                        .try_reserve_exact(native.holes.len())
                        .map_err(|_| vm.new_value_error("Hole list reservation refused"))?;
                    for ring in &native.holes {
                        hole_lists.push(build_ring(ring)?.into());
                    }
                    let face = Face {
                        r#loop: PyRwLock::new(r#loop),
                        hole_loops: PyRwLock::new(vm.ctx.new_list(hole_lists)),
                        attrs: PyRwLock::new(vm.ctx.new_dict().into()),
                    }
                    .into_ref(&vm.ctx);
                    drop(vertices);
                    let mut edges=zelf.edges.borrow_vec_mut();
                    if edges.len()!=transaction.base_edge_extent()+entry.edges_created.len(){return Err(vm.new_value_error("Edge list ownership mismatch"));}
                    edges.try_reserve(new_edges).map_err(|_|vm.new_value_error("Edge list reservation refused"))?;
                    drop(edges);
                    for &(a,b) in &transaction.added_edge_endpoints()[entry.edges_created.len()..] {
                        let vertices=zelf.vertices.borrow_vec();
                        let a=vertices.get(a).ok_or_else(||vm.new_value_error("Edge endpoint outside membership"))?.clone().downcast::<Vertex>().map_err(|_|vm.new_value_error("Edge endpoint requires Vertex"))?;
                        let b=vertices.get(b).ok_or_else(||vm.new_value_error("Edge endpoint outside membership"))?.clone().downcast::<Vertex>().map_err(|_|vm.new_value_error("Edge endpoint requires Vertex"))?;
                        drop(vertices);
                        let edge=Edge::generated(a.clone(),b.clone(),vm);
                        let probe=GRAPH_REENTRY_PROBE.with(|p|p.replace(0));
                        if probe==1 {
                            // Cold engineering callback reentry while the stage is exclusively borrowed.
                            let _caught=vm.call_method(zelf.as_object(), "add_face", (vm.ctx.new_list(vec![]),));
                        } else if probe==2 {
                            let constructor=zelf.as_object().get_attr("__class__", vm)?;
                            let _caught=constructor.call((),vm);
                        }
                        for v in [&a,&b] {let set=v.edges.read().clone().ok_or_else(||vm.new_value_error("Cleared Vertex incidence"))?.downcast::<PySet>().map_err(|_|vm.new_value_error("Vertex incidence requires set"))?;set.add(edge.clone().into(),vm)?;}
                        let mut edges=zelf.edges.borrow_vec_mut();
                        if edges.len()!=transaction.base_edge_extent()+entry.edges_created.len(){return Err(vm.new_value_error("Edge membership changed during incidence callback"));}
                        edges.push(edge.clone().into());entry.edges_created.push(edge);
                    }
                    for &edge_id in transaction.staged_face_edges(id).map_err(|e|vm.new_value_error(e))? {
                        let edge={let edges=zelf.edges.borrow_vec(); edges.get(edge_id).ok_or_else(||vm.new_value_error("Face Edge outside membership"))?.clone().downcast::<Edge>().map_err(|_|vm.new_value_error("Face Edge requires Edge"))?};
                        let radial=edge.faces.read().clone().ok_or_else(||vm.new_value_error("Cleared Edge radial"))?.downcast::<rustpython_vm::builtins::PyList>().map_err(|_|vm.new_value_error("Edge faces requires list"))?;
                        let mut values=radial.borrow_vec_mut();if !values.iter().any(|v|v.is(&face)){values.try_reserve(1).map_err(|_|vm.new_value_error("Edge radial reservation refused"))?;values.push(face.clone().into());}
                    }

                    let mut faces = zelf.faces.borrow_vec_mut();
                    faces
                        .try_reserve(1)
                        .map_err(|_| vm.new_value_error("Face list reservation refused"))?;
                    faces.push(face.clone().into());
                    entry.faces_created.push(face.clone());
                    Ok(face)
                })();
                if operation.is_err() {
                    stage.failed = true;
                } else if !zelf.managed.get() {
                    let mut entry = stage.entries.remove(&zelf.get_id()).unwrap();
                    let published = zelf
                        .owner
                        .write()
                        .commit(entry.transaction.take().unwrap())
                        .map_err(|e| vm.new_value_error(e));
                    if let Err(error) = published {
                        stage.failed = true;
                        return Err(error);
                    }
                    entry.published = true;
                }
                operation
            });
            result
        }
    }
}
struct IdentityOrder {
    order: Vec<usize>,
    changed: bool,
    _charge: Lease,
}
fn identity_order(
    target: &[rustpython_vm::PyObjectRef],
    current: &[rustpython_vm::PyObjectRef],
    budget: &OwnerBudget,
) -> Result<IdentityOrder, String> {
    use rustpython_vm::AsObject;
    if target.len() != current.len() {
        return Err("Identity permutation requires unchanged membership extent".into());
    }
    let bytes = target
        .len()
        .checked_mul(128)
        .ok_or("Identity permutation preparation overflow")?;
    let charge = budget.reserve(Kind::Preparation, bytes)?;
    let mut ids = HashMap::new();
    ids.try_reserve(current.len())
        .map_err(|_| "Identity permutation object map refused")?;
    for (id, obj) in current.iter().enumerate() {
        if ids.insert(obj.get_id(), id).is_some() {
            return Err("Identity permutation repeated current object".into());
        }
    }
    let mut seen = Vec::new();
    seen.try_reserve_exact(current.len())
        .map_err(|_| "Identity permutation validation refused")?;
    seen.resize(current.len(), false);
    let mut order = Vec::new();
    order
        .try_reserve_exact(target.len())
        .map_err(|_| "Identity permutation order refused")?;
    let mut changed = false;
    for (new, obj) in target.iter().enumerate() {
        let old = *ids
            .get(&obj.get_id())
            .ok_or("Identity permutation replaces an existing object")?;
        if seen[old] {
            return Err("Identity permutation repeats target object".into());
        }
        seen[old] = true;
        changed |= old != new;
        order.push(old);
    }
    Ok(IdentityOrder {
        order,
        changed,
        _charge: charge,
    })
}
struct Projection {
    positions: Vec<glam::DVec3>,
    loops: Vec<(Vec<usize>, Vec<Vec<usize>>)>,
    _charge: Lease,
}
impl Projection {
    fn prepare(
        mesh: &PyRef<editable_mesh::Mesh>,
        budget: &OwnerBudget,
    ) -> Result<Option<Self>, String> {
        Self::prepare_inner(mesh, budget, false, false)
    }
    fn prepare_prefix(
        mesh: &PyRef<editable_mesh::Mesh>,
        budget: &OwnerBudget,
        append: bool,
    ) -> Result<Option<Self>, String> {
        Self::prepare_inner(mesh, budget, append, false)
    }
    fn prepare_inner(
        mesh: &PyRef<editable_mesh::Mesh>,
        budget: &OwnerBudget,
        append: bool,
        force: bool,
    ) -> Result<Option<Self>, String> {
        use rustpython_vm::AsObject;
        let owner = mesh.owner.read();
        let native = owner.mesh();
        let vertices = mesh.vertices.borrow_vec();
        let faces = mesh.faces.borrow_vec();
        if (!append
            && (vertices.len() != native.vertices.len() || faces.len() != native.faces.len()))
            || vertices.len() < native.vertices.len()
            || faces.len() < native.faces.len()
        {
            return Err("Existing field projection requires unchanged membership".into());
        }
        let slots = vertices
            .len()
            .checked_mul(128)
            .and_then(|n| faces.len().checked_mul(128).and_then(|m| n.checked_add(m)))
            .ok_or("Projection extent overflow")?;
        let mut charge = budget.reserve(Kind::Preparation, slots)?;
        let mut ids = HashMap::new();
        ids.try_reserve(vertices.len())
            .map_err(|_| "Projection identity map refused")?;
        let mut positions = Vec::new();
        positions
            .try_reserve_exact(vertices.len())
            .map_err(|_| "Projection positions refused")?;
        let mut changed = false;
        for (id, value) in vertices.iter().enumerate() {
            if value.downcast_ref::<editable_mesh::Vertex>().is_none() {
                return Err("Projection vertex requires Vertex".into());
            }
            if ids.insert(value.get_id(), id).is_some() {
                return Err("Repeated Vertex membership projection unsupported".into());
            }
        }
        for (id, value) in vertices.iter().take(native.vertices.len()).enumerate() {
            let v = value.downcast_ref::<editable_mesh::Vertex>().unwrap();
            let p = v.position.read().coordinates();
            let p = glam::DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64);
            changed |= p != native.vertices[id].position;
            positions.push(p);
        }
        let mut loops = Vec::new();
        loops
            .try_reserve_exact(faces.len())
            .map_err(|_| "Projection face slots refused")?;
        let mut bytes = slots;
        for (id, value) in faces.iter().take(native.faces.len()).enumerate() {
            let f = value
                .downcast_ref::<editable_mesh::Face>()
                .ok_or("Projection face requires Face")?;
            let outer = f.r#loop.read().clone();
            let outer = outer.borrow_vec();
            let holes = f.hole_loops.read().clone();
            let holes = holes.borrow_vec();
            let mut members = outer
                .len()
                .checked_add(holes.len())
                .ok_or("Projection member overflow")?;
            for h in holes.iter() {
                let h = h
                    .downcast_ref::<rustpython_vm::builtins::PyList>()
                    .ok_or("Projection hole requires list")?;
                members = members
                    .checked_add(h.borrow_vec().len())
                    .ok_or("Projection hole overflow")?;
            }
            bytes = bytes
                .checked_add(members.checked_mul(32).ok_or("Projection loop overflow")?)
                .ok_or("Projection loop overflow")?;
            charge.resize(bytes)?;
            let map = |values: &[rustpython_vm::PyObjectRef]| -> Result<Vec<usize>, String> {
                let mut ring = Vec::new();
                ring.try_reserve_exact(values.len())
                    .map_err(|_| "Projection ring refused")?;
                for v in values {
                    ring.push(
                        *ids.get(&v.get_id())
                            .ok_or("Face loop references Vertex outside membership")?,
                    );
                }
                Ok(ring)
            };
            let outer = map(&outer)?;
            let mut prepared_holes = Vec::new();
            prepared_holes
                .try_reserve_exact(holes.len())
                .map_err(|_| "Projection hole slots refused")?;
            for h in holes.iter() {
                prepared_holes.push(map(&h
                    .downcast_ref::<rustpython_vm::builtins::PyList>()
                    .unwrap()
                    .borrow_vec())?);
            }
            changed |=
                outer != native.faces[id].vertices || prepared_holes != native.faces[id].holes;
            loops.push((outer, prepared_holes));
        }
        if changed || force {
            Ok(Some(Self {
                positions,
                loops,
                _charge: charge,
            }))
        } else {
            Ok(None)
        }
    }
}

struct VertexRestore {
    vertex: PyRef<editable_mesh::Vertex>,
    position: PyRef<crate::vm_groups::native_groups::QVector3D>,
}
struct FaceRestore {
    face: PyRef<editable_mesh::Face>,
    outer: rustpython_vm::builtins::PyListRef,
    holes: rustpython_vm::builtins::PyListRef,
    attrs: rustpython_vm::PyObjectRef,
}
struct RootRestore<'a> {
    vm: &'a rustpython_vm::VirtualMachine,
    mesh: PyRef<editable_mesh::Mesh>,
    vertices: Vec<rustpython_vm::PyObjectRef>,
    faces: Vec<rustpython_vm::PyObjectRef>,
    positions: Vec<VertexRestore>,
    graph: GraphState,
    face_fields: Vec<FaceRestore>,
    _charge: Option<Lease>,
    native_state: uuid::Uuid,
    armed: bool,
}
impl<'a> RootRestore<'a> {
    fn prepare(
        mesh: PyRef<editable_mesh::Mesh>,
        budget: &OwnerBudget,
        vm: &'a rustpython_vm::VirtualMachine,
    ) -> Result<Self, String> {
        use rustpython_vm::{builtins::PyDict, PyPayload};
        fn copy(
            list: &rustpython_vm::builtins::PyListRef,
        ) -> Result<Vec<rustpython_vm::PyObjectRef>, String> {
            let source = list.borrow_vec();
            let mut out = Vec::new();
            out.try_reserve_exact(source.len())
                .map_err(|_| "Root reference preparation refused")?;
            out.extend(source.iter().cloned());
            Ok(out)
        }
        let bytes = mesh
            .vertices
            .borrow_vec()
            .len()
            .checked_mul(
                std::mem::size_of::<VertexRestore>()
                    + std::mem::size_of::<rustpython_vm::PyObjectRef>(),
            )
            .and_then(|n| {
                mesh.faces
                    .borrow_vec()
                    .len()
                    .checked_mul(
                        std::mem::size_of::<FaceRestore>()
                            + std::mem::size_of::<rustpython_vm::PyObjectRef>(),
                    )
                    .and_then(|m| n.checked_add(m))
            })
            .ok_or("Root restoration extent overflow")?;
        let mut charge = budget.reserve(Kind::Preparation, bytes)?;
        let vertices = copy(&mesh.vertices)?;
        let faces = copy(&mesh.faces)?;
        let mut positions = Vec::new();
        positions
            .try_reserve_exact(vertices.len())
            .map_err(|_| "Position restoration records refused")?;
        for value in &vertices {
            let vertex = value
                .clone()
                .downcast::<editable_mesh::Vertex>()
                .map_err(|_| "Root vertex list requires Vertex")?;
            let p = vertex.position.read().coordinates();
            let position =
                crate::vm_groups::native_groups::QVector3D::from_coordinates(p).into_ref(&vm.ctx);
            positions.push(VertexRestore { vertex, position });
        }
        let mut face_fields = Vec::new();
        face_fields
            .try_reserve_exact(faces.len())
            .map_err(|_| "Face restoration records refused")?;
        let mut bytes = bytes;
        for value in &faces {
            let face = value
                .clone()
                .downcast::<editable_mesh::Face>()
                .map_err(|_| "Root face list requires Face")?;
            let outer_len = face.r#loop.read().borrow_vec().len();
            let source_holes = face.hole_loops.read().clone();
            let source_holes = source_holes.borrow_vec();
            let mut slots = outer_len
                .checked_add(source_holes.len())
                .ok_or("Loop extent overflow")?;
            for h in source_holes.iter() {
                let h = h
                    .downcast_ref::<rustpython_vm::builtins::PyList>()
                    .ok_or("Hole requires list")?;
                slots = slots
                    .checked_add(h.borrow_vec().len())
                    .ok_or("Hole extent overflow")?;
            }
            // Logical member-slot admission precedes every native vector allocation.
            bytes = bytes
                .checked_add(
                    slots
                        .checked_mul(std::mem::size_of::<rustpython_vm::PyObjectRef>())
                        .ok_or("Loop byte overflow")?,
                )
                .ok_or("Loop byte overflow")?;
            charge.resize(bytes)?;
            let outer = copy(&face.r#loop.read())?;
            let mut extra = outer
                .capacity()
                .checked_sub(outer_len)
                .ok_or("Loop capacity mismatch")?;
            let mut holes = Vec::new();
            holes
                .try_reserve_exact(source_holes.len())
                .map_err(|_| "Hole restoration slots refused")?;
            extra = extra
                .checked_add(holes.capacity() - source_holes.len())
                .ok_or("Hole capacity overflow")?;
            for h in source_holes.iter() {
                let h = h
                    .downcast_ref::<rustpython_vm::builtins::PyList>()
                    .ok_or("Hole requires list")?;
                let h = h.borrow_vec();
                let mut ring = Vec::new();
                ring.try_reserve_exact(h.len())
                    .map_err(|_| "Hole restoration members refused")?;
                extra = extra
                    .checked_add(ring.capacity() - h.len())
                    .ok_or("Hole capacity overflow")?;
                ring.extend(h.iter().cloned());
                holes.push(vm.ctx.new_list(ring).into());
            }
            // Allocator slack is reconciled before execution, not claimed physically capped.
            bytes = bytes
                .checked_add(
                    extra
                        .checked_mul(std::mem::size_of::<rustpython_vm::PyObjectRef>())
                        .ok_or("Loop capacity overflow")?,
                )
                .ok_or("Loop capacity overflow")?;
            charge.resize(bytes)?;
            let old = face.attrs.read().clone();
            let old = old
                .downcast::<PyDict>()
                .map_err(|_| "Root face attrs require dict")?;
            let attrs = old.copy().into_ref(&vm.ctx).into();
            face_fields.push(FaceRestore {
                face,
                outer: vm.ctx.new_list(outer),
                holes: vm.ctx.new_list(holes),
                attrs,
            });
        }
        let graph=GraphState::capture(&mesh,budget,vm)?;
        let native_state = mesh.owner.read().state_token();
        Ok(Self {
            vm,
            mesh,
            vertices,
            faces,
            positions,
            graph,
            face_fields,
            _charge: Some(charge),
            native_state,
            armed: true,
        })
    }
    fn validate_live(&self, append: bool) -> Result<bool, String> {
        self.validate_live_membership(append, false)
    }
    fn validate_live_membership(&self, append: bool, permutation: bool) -> Result<bool, String> {
        use rustpython_vm::AsObject;
        let vertices = self.mesh.vertices.borrow_vec();
        let faces = self.mesh.faces.borrow_vec();
        if vertices.len() < self.vertices.len()
            || faces.len() < self.faces.len()
            || (!append
                && (vertices.len() != self.vertices.len() || faces.len() != self.faces.len()))
        {
            return Err("Existing field publication membership extent unsupported".into());
        }
        let reordered = vertices.iter().zip(&self.vertices).any(|(a, b)| !a.is(b))
            || faces.iter().zip(&self.faces).any(|(a, b)| !a.is(b));
        if reordered && permutation && !append {
            let budget = self.mesh.owner.read().budget();
            let _vertices = identity_order(&vertices, &self.vertices, &budget)?;
            let _faces = identity_order(&faces, &self.faces, &budget)?;
        } else if reordered {
            return Err(
                "Existing field publication requires exact Vertex/Face identity order".into(),
            );
        }
        let mut changed = reordered;
        self.graph.validate_membership(&self.mesh,append)?;

        for record in &self.positions {
            changed |= record.vertex.position.read().coordinates() != record.position.coordinates();
        }
        for record in &self.face_fields {
            let current = record.face.r#loop.read().clone();
            let current = current.borrow_vec();
            let before = record.outer.borrow_vec();
            changed |= current.len() != before.len()
                || current.iter().zip(before.iter()).any(|(a, b)| !a.is(b));
            let holes = record.face.hole_loops.read().clone();
            let holes = holes.borrow_vec();
            let old = record.holes.borrow_vec();
            if holes.len() != old.len() {
                changed = true;
            } else {
                for (a, b) in holes.iter().zip(old.iter()) {
                    let a = a
                        .downcast_ref::<rustpython_vm::builtins::PyList>()
                        .ok_or("Live hole requires list")?;
                    let b = b
                        .downcast_ref::<rustpython_vm::builtins::PyList>()
                        .ok_or("Captured hole requires list")?;
                    let a = a.borrow_vec();
                    let b = b.borrow_vec();
                    changed |= a.len() != b.len() || a.iter().zip(b.iter()).any(|(a, b)| !a.is(b));
                }
            }
        }
        Ok(changed)
    }
    fn apply(&mut self) {
        self.vm.host_budget_abort();
        let vertices = std::mem::take(&mut self.vertices);
        let faces = std::mem::take(&mut self.faces);
        let old = std::mem::replace(&mut *self.mesh.vertices.borrow_vec_mut(), vertices);
        drop(old);
        let old = std::mem::replace(&mut *self.mesh.faces.borrow_vec_mut(), faces);
        drop(old);
        self.graph.restore(&self.mesh);
        for record in self.positions.drain(..) {
            let old = std::mem::replace(&mut *record.vertex.position.write(), record.position);
            drop(old);
        }
        for record in self.face_fields.drain(..) {
            let old = std::mem::replace(&mut *record.face.r#loop.write(), record.outer);
            drop(old);
            let old = std::mem::replace(&mut *record.face.hole_loops.write(), record.holes);
            drop(old);
            let old = std::mem::replace(&mut *record.face.attrs.write(), record.attrs);
            drop(old);
        }
        self.armed = false;
    }
    fn restore(mut self) {
        self.apply();
    }
    fn dismiss(mut self) {
        self.armed = false;
    }
}
impl Drop for RootRestore<'_> {
    fn drop(&mut self) {
        if self.armed {
            self.apply();
        }
    }
}

struct RootSnapshot {
    mesh: PyRef<editable_mesh::Mesh>,
    vertices: Vec<rustpython_vm::PyObjectRef>,
    faces: Vec<rustpython_vm::PyObjectRef>,
    positions: Vec<VertexRestore>,
    graph: GraphState,
    fields: Vec<FaceRestore>,
    native_state: uuid::Uuid,
    _charge: Lease,
}
struct HistoryCommand {
    before: RootSnapshot,
    after: RootSnapshot,
    append: bool,
}
struct HistoryCapacity {
    values: Vec<HistoryCommand>,
    charge: Lease,
}
impl HistoryCapacity {
    fn prepare(values: &Vec<HistoryCommand>, budget: &OwnerBudget) -> Result<Option<Self>, String> {
        let needed = values
            .len()
            .checked_add(1)
            .ok_or("History extent overflow")?;
        if needed <= values.capacity() {
            return Ok(None);
        }
        let capacity = values
            .capacity()
            .checked_mul(2)
            .ok_or("History capacity overflow")?
            .max(4)
            .max(needed);
        let mut charge = budget.reserve(
            Kind::Preparation,
            capacity
                .checked_mul(std::mem::size_of::<HistoryCommand>())
                .ok_or("History slot byte overflow")?,
        )?;
        let mut prepared = Vec::new();
        prepared
            .try_reserve_exact(capacity)
            .map_err(|_| "History slot preparation refused")?;
        charge.resize(
            prepared
                .capacity()
                .checked_mul(std::mem::size_of::<HistoryCommand>())
                .ok_or("History slot capacity overflow")?,
        )?;
        Ok(Some(Self {
            values: prepared,
            charge,
        }))
    }
    fn apply(mut self, values: &mut Vec<HistoryCommand>, charge: &mut Lease) {
        let mut old = std::mem::replace(values, self.values);
        values.append(&mut old);
        drop(old);
        self.charge
            .reclassify(Kind::History)
            .expect("Admitted same-ledger slot transfer");
        *charge = self.charge;
    }
}
impl RootSnapshot {
    fn thaw<'a>(
        &self,
        vm: &'a rustpython_vm::VirtualMachine,
        budget: &OwnerBudget,
    ) -> Result<RootRestore<'a>, String> {
        use rustpython_vm::{builtins::PyDict, PyPayload};
        let charge = budget.reserve(Kind::Preparation, self._charge.bytes())?;
        let mut vertices = Vec::new();
        vertices
            .try_reserve_exact(self.vertices.len())
            .map_err(|_| "History vertex references refused")?;
        vertices.extend(self.vertices.iter().cloned());
        let mut faces = Vec::new();
        faces
            .try_reserve_exact(self.faces.len())
            .map_err(|_| "History face references refused")?;
        faces.extend(self.faces.iter().cloned());
        let mut positions = Vec::new();
        positions
            .try_reserve_exact(self.positions.len())
            .map_err(|_| "History positions refused")?;
        for p in &self.positions {
            positions.push(VertexRestore {
                vertex: p.vertex.clone(),
                position: crate::vm_groups::native_groups::QVector3D::from_coordinates(
                    p.position.coordinates(),
                )
                .into_ref(&vm.ctx),
            });
        }
        let copy=|list:&rustpython_vm::builtins::PyListRef|->Result<rustpython_vm::builtins::PyListRef,String>{let list=list.borrow_vec();let mut values=Vec::new();values.try_reserve_exact(list.len()).map_err(|_|"History loop references refused")?;values.extend(list.iter().cloned());Ok(vm.ctx.new_list(values))};
        let mut fields = Vec::new();
        fields
            .try_reserve_exact(self.fields.len())
            .map_err(|_| "History face fields refused")?;
        for f in &self.fields {
            let outer = copy(&f.outer)?;
            let holes = f.holes.borrow_vec();
            let mut rings = Vec::new();
            rings
                .try_reserve_exact(holes.len())
                .map_err(|_| "History holes refused")?;
            for h in holes.iter() {
                let h = h
                    .clone()
                    .downcast::<rustpython_vm::builtins::PyList>()
                    .map_err(|_| "History hole requires list")?;
                rings.push(copy(&h)?.into());
            }
            let attrs = f
                .attrs
                .clone()
                .downcast::<PyDict>()
                .map_err(|_| "History attrs require dict")?
                .copy()
                .into_ref(&vm.ctx)
                .into();
            fields.push(FaceRestore {
                face: f.face.clone(),
                outer,
                holes: vm.ctx.new_list(rings),
                attrs,
            });
        }
        Ok(RootRestore {
            vm,
            mesh: self.mesh.clone(),
            vertices,
            faces,
            positions,
            graph: self.graph.copy(budget,vm)?,
            face_fields: fields,
            _charge: Some(charge),
            native_state: self.native_state,
            armed: false,
        })
    }
    fn geometry(&self, budget: &OwnerBudget) -> Result<Projection, String> {
        use rustpython_vm::AsObject;
        let charge = budget.reserve(Kind::Preparation, self._charge.bytes())?;
        let mut ids = HashMap::new();
        ids.try_reserve(self.vertices.len())
            .map_err(|_| "History identity map refused")?;
        for (i, v) in self.vertices.iter().enumerate() {
            if ids.insert(v.get_id(), i).is_some() {
                return Err("History repeated membership unsupported".into());
            }
        }
        let mut positions = Vec::new();
        positions
            .try_reserve_exact(self.positions.len())
            .map_err(|_| "History projection positions refused")?;
        for p in &self.positions {
            let p = p.position.coordinates();
            positions.push(glam::DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64));
        }
        let copy = |ring: &rustpython_vm::builtins::PyListRef| -> Result<Vec<usize>, String> {
            let ring = ring.borrow_vec();
            let mut ids_out = Vec::new();
            ids_out
                .try_reserve_exact(ring.len())
                .map_err(|_| "History projection loop refused")?;
            for v in ring.iter() {
                ids_out.push(
                    *ids.get(&v.get_id())
                        .ok_or("History Vertex identity outside membership")?,
                );
            }
            Ok(ids_out)
        };
        let mut loops = Vec::new();
        loops
            .try_reserve_exact(self.fields.len())
            .map_err(|_| "History projection faces refused")?;
        for f in &self.fields {
            let outer = copy(&f.outer)?;
            let holes = f.holes.borrow_vec();
            let mut rings = Vec::new();
            rings
                .try_reserve_exact(holes.len())
                .map_err(|_| "History projection holes refused")?;
            for h in holes.iter() {
                rings.push(copy(
                    &h.clone()
                        .downcast::<rustpython_vm::builtins::PyList>()
                        .map_err(|_| "History hole requires list")?,
                )?);
            }
            loops.push((outer, rings));
        }
        Ok(Projection {
            positions,
            loops,
            _charge: charge,
        })
    }
}
impl RootRestore<'_> {
    fn freeze(mut self) -> RootSnapshot {
        self.armed = false;
        let mut charge = self._charge.take().expect("Exclusive history lease");
        charge
            .reclassify(Kind::History)
            .expect("Same ledger history lease");
        RootSnapshot {
            mesh: self.mesh.clone(),
            vertices: std::mem::take(&mut self.vertices),
            faces: std::mem::take(&mut self.faces),
            positions: std::mem::take(&mut self.positions),
            graph: self.graph.take_for_history(),
            fields: std::mem::take(&mut self.face_fields),
            native_state: self.native_state,
            _charge: charge,
        }
    }
    fn equivalent(&self, other:&Self)->rustpython_vm::PyResult<bool> {
        use rustpython_vm::AsObject;
        if self.vertices.len()!=other.vertices.len()||self.vertices.iter().zip(&other.vertices).any(|(a,b)|!a.is(b)){return Ok(false);}
        if !self.graph.members_equivalent(&other.graph){return Ok(false);}
        if self.faces.len()!=other.faces.len()||self.faces.iter().zip(&other.faces).any(|(a,b)|!a.is(b)){return Ok(false);}
        for (a,b) in self.positions.iter().zip(&other.positions){if a.position.coordinates()!=b.position.coordinates(){return Ok(false);}}
        if !self.graph.equivalent(&other.graph,self.vm)?{return Ok(false);}
        // Source groups field families by snapshot dictionary key, not by Face record.
        for (a,b) in self.face_fields.iter().zip(&other.face_fields){if !self.vm.bool_eq(a.outer.as_object(),b.outer.as_object())?{return Ok(false);}}
        for (a,b) in self.face_fields.iter().zip(&other.face_fields){if !self.vm.bool_eq(a.holes.as_object(),b.holes.as_object())?{return Ok(false);}}
        for (a,b) in self.face_fields.iter().zip(&other.face_fields){if !self.vm.bool_eq(a.attrs.as_object(),b.attrs.as_object())?{return Ok(false);}}
        Ok(true)
    }

}

pub struct Controller {
    history_enabled: bool,
    undo: Vec<HistoryCommand>,
    redo: Vec<HistoryCommand>,
    undo_slots: Lease,
    redo_slots: Lease,
    interpreter: Interpreter,
    scope: Scope,
    budget: OwnerBudget,
    tainted: bool,
    pressure_after: Option<usize>,
    last_prepared: usize,
    panic_after_execution: bool,
    pressure_after_script: bool,
}
impl Controller {
    pub fn new() -> Self {
        let builder = InterpreterBuilder::new();
        let vectors = crate::vm_groups::native_groups::module_def(&builder.ctx);
        let meshes = editable_mesh::module_def(&builder.ctx);
        let interpreter = builder
            .add_native_module(vectors)
            .add_native_module(meshes)
            .interpreter();
        let scope=interpreter.enter(|vm|{let s=vm.new_scope_with_builtins();vm.run_code_obj(vm.compile("from native_groups import QVector3D\nfrom editable_mesh import Mesh,Vertex,Edge,Face",Mode::Exec,"<setup>").unwrap(),s.clone()).unwrap();s});
        let budget = OwnerBudget::new(None);
        let mut controller = Self {
            undo_slots: budget.reserve(Kind::History, 0).unwrap(),
            redo_slots: budget.reserve(Kind::History, 0).unwrap(),
            history_enabled: false,
            undo: Vec::new(),
            redo: Vec::new(),
            interpreter,
            scope,
            budget,
            tainted: false,
            pressure_after: None,
            last_prepared: 0,
            panic_after_execution: false,
            pressure_after_script: false,
        };
        controller.execute("mesh=Mesh()").unwrap();
        controller.adopt("mesh");
        controller
    }
    pub fn proof_gc_graph_inventory(&self) -> serde_json::Value {
        self.interpreter.enter(|vm| {
            let objects=vm.state.gc.get_objects(None);
            serde_json::Value::Array(objects.iter().filter(|o| matches!(o.class().name().as_ref(), "Vertex"|"Edge"))
                .map(|o| serde_json::json!({"class":o.class().name().to_string(),"strong_refs_including_inventory":o.strong_count()})).collect())
        })
    }
    pub fn collect_proof_gc(&self) {
        self.interpreter.enter(|vm| {
            vm.state.gc.collect_force(2);
        });
    }
    pub fn proof_automatic_gc(&self, enabled: bool) {
        self.interpreter.enter(|vm| {
            if enabled {
                vm.state.gc.enable();
            } else {
                vm.state.gc.disable();
            }
        });
    }
    pub fn budget(&self) -> OwnerBudget {
        self.budget.clone()
    }
    pub fn pressure_after(&mut self, n: Option<usize>) {
        self.pressure_after = n;
    }
    pub fn last_prepared(&self) -> usize {
        self.last_prepared
    }
    pub fn pressure_after_script(&mut self,value:bool){self.pressure_after_script=value;}
    pub fn graph_reentry_probe(&mut self,kind:u8){ assert!(kind<=2); GRAPH_REENTRY_PROBE.with(|p|p.set(kind)); }
    pub fn panic_after_execution(&mut self, value: bool) {
        self.panic_after_execution = value;
    }
    pub fn tainted(&self) -> bool {
        self.tainted
    }
    pub fn adopt(&self, name: &str) {
        self.interpreter.enter(|vm| {
            self.scope
                .globals
                .get_item(name, vm)
                .unwrap()
                .downcast::<editable_mesh::Mesh>()
                .unwrap()
                .managed
                .set(true);
        });
    }
    pub fn execute(&mut self, source: &str) -> Result<bool, String> {
        if self.tainted || STAGED.with(|s| s.borrow().is_some()) {
            return Err("VM transaction admission refused".into());
        }
        let stage = Stage {
            entries: HashMap::new(),
            budget: self.budget.clone(),
            charge: self.budget.reserve(Kind::Staging, 0)?,
            failed: false,
        };
        self.tainted = true;
        let prior_limit = self.budget.snapshot().limit;
        self.last_prepared = 0;
        let result = self.interpreter.enter(|vm| {
            STAGED.with(|s| *s.borrow_mut() = Some(stage));
            let _guard = Guard(vm);
            vm.host_budget_arm(100000)
                .map_err(|_| "VM fuel admission refused")?;
            let mut baseline = match self.scope.globals.get_item("mesh", vm) {
                Ok(value) => Some(RootRestore::prepare(
                    value
                        .downcast::<editable_mesh::Mesh>()
                        .map_err(|_| "Root binding requires Mesh")?,
                    &self.budget,
                    vm,
                )?),
                Err(_) => None,
            };
            let executed = session::execute(vm, &self.scope, source);
            if self.panic_after_execution {
                panic!("Engineering unwind after VM mutation");
            }
            if self.pressure_after_script { self.budget.set_limit(Some(self.budget.snapshot().total())); }
            let mut stage = STAGED.with(|s| s.borrow_mut().take().unwrap());
            let mut history_after = None;
            let mut history_capacity = None;
            let mut history_changed = false;
            let mut history_append = false;
            let result = if let Err(error) = executed {
                vm.print_exception(&error);
                vm.host_budget_abort();
                stage.rollback();
                Err("Python execution refused".into())
            } else if vm.host_budget_poisoned() || stage.failed {
                vm.host_budget_abort();
                stage.rollback();
                Err("Staged mutation refused".into())
            } else {
                let mut entries = Vec::new();
                entries
                    .try_reserve_exact(stage.entries.len())
                    .map_err(|_| "Publication entries refused")?;
                entries.extend(stage.entries.drain().map(|(_, e)| e));
                let publish = (|| {
                    for entry in &entries {
                        entry.validate_created()?;
                    }
                    if let Some(before) = &baseline {
                        let changed = before
                            .validate_live_membership(!entries.is_empty(), entries.is_empty())?;
                        let _ = changed;
                    }
                    if self.history_enabled {
                        let before = baseline
                            .as_ref()
                            .ok_or("History requires existing root Mesh")?;
                        if entries
                            .iter()
                            .any(|e| !rustpython_vm::AsObject::is(&e.mesh, &before.mesh))
                        {
                            return Err(
                                "Command history currently requires root-only managed publication"
                                    .into(),
                            );
                        }
                        let mut after =
                            RootRestore::prepare(before.mesh.clone(), &self.budget, vm)?;
                        after.armed = false;
                        history_changed = !before
                            .equivalent(&after)
                            .map_err(|_| "History state equality refused")?;
                        if after.validate_live(false)? || !after.graph.identity_live(&after.mesh,&self.budget,vm)? {
                            return Err(
                                "History equality changed geometry after snapshot preparation"
                                    .into(),
                            );
                        }
                        if history_changed {
                            history_capacity = HistoryCapacity::prepare(&self.undo, &self.budget)?;
                            history_append = !entries.is_empty();
                            history_after = Some(after.freeze());
                        }
                    }
                    if entries.is_empty() {
                        if let Ok(value) = self.scope.globals.get_item("mesh", vm) {
                            let mesh = value
                                .downcast::<editable_mesh::Mesh>()
                                .map_err(|_| "Root binding requires Mesh")?;
                            if let Some(before) = baseline.as_ref() {
                                if !rustpython_vm::AsObject::is(&mesh, &before.mesh) {
                                    return Err(
                                        "Root binding replacement projection not yet supported"
                                            .into(),
                                    );
                                }
                                let vertex_order = identity_order(
                                    &mesh.vertices.borrow_vec(),
                                    &before.vertices,
                                    &self.budget,
                                )?;
                                let face_order = identity_order(
                                    &mesh.faces.borrow_vec(),
                                    &before.faces,
                                    &self.budget,
                                )?;
                                let reordered = vertex_order.changed || face_order.changed;
                                if let Some(projection) = Projection::prepare_inner(
                                    &mesh,
                                    &self.budget,
                                    false,
                                    reordered,
                                )? {
                                    let mut owner = mesh.owner.write();
                                    if reordered {
                                        owner
                                            .prepare_membership_as(
                                                &vertex_order.order,
                                                &face_order.order,
                                                projection.positions,
                                                projection.loops,
                                                uuid::Uuid::new_v4(),
                                            )?
                                            .apply();
                                    } else {
                                        owner
                                            .prepare_fields(projection.positions, projection.loops)?
                                            .apply();
                                    }
                                    return Ok(true);
                                }
                                return Ok(if self.history_enabled { history_changed } else {before.validate_live_membership(false,true)? || !before.graph.equivalent_live(&mesh,vm)?});
                            } else if Projection::prepare(&mesh, &self.budget)?.is_some() {
                                return Err("Existing projection requires root baseline".into());
                            }
                        }
                    }
                    let mut transactions = Vec::new();
                    transactions
                        .try_reserve_exact(entries.len())
                        .map_err(|_| "Publication transaction slots refused")?;
                    for entry in &mut entries {
                        transactions.push(entry.transaction.take().unwrap());
                    }
                    let mut projections = Vec::new();
                    projections
                        .try_reserve_exact(entries.len())
                        .map_err(|_| "Mixed projection tickets refused")?;
                    for entry in &entries {
                        projections.push(Projection::prepare_prefix(
                            &entry.mesh,
                            &self.budget,
                            true,
                        )?);
                    }
                    let mut locks = Vec::new();
                    locks
                        .try_reserve_exact(entries.len())
                        .map_err(|_| "Publication owner locks refused")?;
                    for e in &entries {
                        locks.push(e.mesh.owner.write());
                    }
                    let mut prepared = Vec::new();
                    prepared
                        .try_reserve_exact(entries.len())
                        .map_err(|_| "Publication tickets refused")?;
                    for ((owner, transaction), projection) in
                        locks.iter_mut().zip(transactions).zip(projections)
                    {
                        let ticket = owner.prepare(transaction)?;
                        let ticket = match (ticket, projection) {
                            (Some(ticket), Some(p)) => {
                                Some(ticket.with_existing_fields(p.positions, p.loops)?)
                            }
                            (ticket, None) => ticket,
                            (None, Some(_)) => {
                                return Err("Mixed publication missing append ticket".into())
                            }
                        };
                        prepared.push(ticket);
                        self.last_prepared += 1;
                        if self.pressure_after == Some(self.last_prepared) {
                            self.budget.set_limit(Some(self.budget.snapshot().total()));
                        }
                    }
                    for ticket in prepared.into_iter().flatten() {
                        ticket.apply();
                    }
                    Ok::<_, String>(!entries.is_empty())
                })();
                if publish.is_err() {
                    vm.host_budget_abort();
                    for e in &entries {
                        discard_tail(&e.mesh.faces, e.faces);
                        discard_tail(&e.mesh.vertices, e.vertices);
                    }
                } else {
                    for e in &mut entries {
                        e.published = true;
                    }
                }
                publish
            };
            if let Some(before) = baseline.take() {
                if result.is_err() {
                    before.restore();
                } else if self.history_enabled && history_changed {
                    let mut after = history_after.take().expect("Prepared after snapshot");
                    after.native_state = after.mesh.owner.read().state_token();
                    if let Some(capacity) = history_capacity.take() {
                        capacity.apply(&mut self.undo, &mut self.undo_slots);
                    }
                    vm.host_budget_abort();
                    self.redo.clear();
                    self.undo.push(HistoryCommand {
                        before: before.freeze(),
                        after,
                        append: history_append,
                    });
                } else {
                    before.dismiss();
                }
            }
            let result = if result.is_ok() && self.history_enabled {
                Ok(history_changed)
            } else {
                result
            };
            vm.host_budget_reset_after_discard();
            result
        });
        self.interpreter
            .enter(|vm| vm.host_budget_reset_after_discard());
        self.budget.set_limit(prior_limit);
        self.tainted = false;
        result
    }
    pub fn enable_history(&mut self) {
        assert!(self.undo.is_empty() && self.redo.is_empty());
        self.history_enabled = true;
    }
    pub fn history_counts(&self) -> (usize, usize) {
        (self.undo.len(), self.redo.len())
    }
    pub fn undo(&mut self) -> Result<bool, String> {
        self.restore_command(false)
    }
    pub fn redo(&mut self) -> Result<bool, String> {
        self.restore_command(true)
    }
    fn restore_command(&mut self, redo: bool) -> Result<bool, String> {
        if self.tainted || STAGED.with(|s| s.borrow().is_some()) {
            return Err("History admission refused".into());
        }
        if !self.history_enabled {
            return Err("Controller history disabled".into());
        }
        if (redo && self.redo.is_empty()) || (!redo && self.undo.is_empty()) {
            return Ok(false);
        }
        let capacity =
            HistoryCapacity::prepare(if redo { &self.undo } else { &self.redo }, &self.budget)?;
        let command = if redo {
            self.redo.last()
        } else {
            self.undo.last()
        };
        let Some(command) = command else {
            return Ok(false);
        };
        let target = if redo {
            &command.after
        } else {
            &command.before
        };
        let result = self.interpreter.enter(|vm| {
            let restore = target.thaw(vm, &self.budget)?;
            let projection = if command.append {
                None
            } else {
                Some(target.geometry(&self.budget)?)
            };
            if let Some(p) = projection {
                let vertex_order = identity_order(
                    &target.vertices,
                    &target.mesh.vertices.borrow_vec(),
                    &self.budget,
                )?;
                let face_order =
                    identity_order(&target.faces, &target.mesh.faces.borrow_vec(), &self.budget)?;
                let mut owner = target.mesh.owner.write();
                if vertex_order.changed || face_order.changed {
                    owner
                        .prepare_membership_as(
                            &vertex_order.order,
                            &face_order.order,
                            p.positions,
                            p.loops,
                            target.native_state,
                        )?
                        .apply();
                } else {
                    owner
                        .prepare_fields_as(p.positions, p.loops, target.native_state)?
                        .apply();
                }
            } else {
                let changed = if redo {
                    target.mesh.owner.write().redo()?
                } else {
                    target.mesh.owner.write().undo()?
                };
                if !changed {
                    return Err("Native append history missing".into());
                }
            }
            restore.restore();
            vm.host_budget_reset_after_discard();
            Ok::<_, String>(true)
        });
        if result.is_ok() {
            if redo {
                if let Some(capacity) = capacity {
                    capacity.apply(&mut self.undo, &mut self.undo_slots);
                }
                let c = self.redo.pop().unwrap();
                self.undo.push(c);
            } else {
                if let Some(capacity) = capacity {
                    capacity.apply(&mut self.redo, &mut self.redo_slots);
                }
                let c = self.undo.pop().unwrap();
                self.redo.push(c);
            }
        }
        result
    }
    pub fn assert_script(&mut self, code: &str) {
        if let Err(e) = self.execute(code) {
            panic!("{e}")
        }
    }
    pub fn observe(&self, name: &str) -> serde_json::Value {
        self.interpreter.enter(|vm|{use rustpython_vm::AsObject;let mesh=self.scope.globals.get_item("m",vm).unwrap().downcast::<editable_mesh::Mesh>().unwrap();let owner=mesh.owner.read();let m=owner.mesh();let held=self.scope.globals.get_item("held",vm).unwrap().downcast::<editable_mesh::Face>().unwrap();let old_loop=self.scope.globals.get_item("loop",vm).unwrap();let attrs=self.scope.globals.get_item("heldattrs",vm).unwrap();let position=self.scope.globals.get_item("heldposition",vm).unwrap();let live=mesh.faces.borrow_vec();let first=held.r#loop.read().borrow_vec()[0].clone().downcast::<editable_mesh::Vertex>().unwrap();serde_json::json!({"name":name,"mesh":{"vertices":m.vertices.iter().map(|v|v.position.to_array()).collect::<Vec<_>>(),"edges":m.edges.iter().map(|e|[e.v0,e.v1]).collect::<Vec<_>>(),"faces":m.faces.iter().map(|f|serde_json::json!({"vertices":f.vertices,"holes":f.holes})).collect::<Vec<_>>()},"persistent_faces":mesh.faces.is(&self.scope.globals.get_item("faces",vm).unwrap()),"persistent_vertices":mesh.vertices.is(&self.scope.globals.get_item("vertices",vm).unwrap()),"held_face_live":live.iter().any(|v|v.is(&held)),"face_loop_preserved":held.r#loop.read().is(&old_loop),"attrs_dict_preserved":held.attrs.read().is(&attrs),"vertex_position_preserved":first.position.read().is(&position)})})
    }
    pub fn observe_history(&self, name: &str) -> serde_json::Value {
        self.interpreter.enter(|vm|{
       use rustpython_vm::AsObject;
       let mesh=self.scope.globals.get_item("m",vm).unwrap().downcast::<editable_mesh::Mesh>().unwrap();let f=self.scope.globals.get_item("held",vm).unwrap().downcast::<editable_mesh::Face>().unwrap();let v=self.scope.globals.get_item("v",vm).unwrap().downcast::<editable_mesh::Vertex>().unwrap();let oldloop=self.scope.globals.get_item("oldloop",vm).unwrap();let oldattrs=self.scope.globals.get_item("oldattrs",vm).unwrap();let oldposition=self.scope.globals.get_item("oldposition",vm).unwrap().downcast::<crate::vm_groups::native_groups::QVector3D>().unwrap();let shared=self.scope.globals.get_item("shared",vm).unwrap();let attrs=f.attrs.read().clone().downcast::<rustpython_vm::builtins::PyDict>().unwrap();let value=attrs.get_item("shared",vm).unwrap();let strings=value.downcast_ref::<rustpython_vm::builtins::PyList>().unwrap().borrow_vec().iter().map(|x|x.downcast_ref::<rustpython_vm::builtins::PyStr>().unwrap().as_utf8().unwrap().as_str().to_owned()).collect::<Vec<_>>();let owner=mesh.owner.read();let native=owner.mesh();let faces=mesh.faces.borrow_vec();let vertices=mesh.vertices.borrow_vec();
       serde_json::json!({"name":name,"positions":native.vertices.iter().map(|v|v.position.to_array()).collect::<Vec<_>>(),"faces":native.faces.iter().map(|f|f.vertices.clone()).collect::<Vec<_>>(),"face_live":faces.iter().any(|x|x.is(&f)),"vertex_live":vertices.iter().any(|x|x.is(&v)),"loop_original":f.r#loop.read().is(&oldloop),"position_original":v.position.read().is(&oldposition),"attrs_original":f.attrs.read().is(&oldattrs),"shared_value":strings,"shared_identity":value.is(&shared),"held_position":oldposition.coordinates(),"undo":self.undo.len(),"redo":self.redo.len()})
      })
    }
    pub fn observe_graph_order(&self)->serde_json::Value {
        self.interpreter.enter(|vm| {
            let events=self.scope.globals.get_item("events",vm).unwrap().downcast::<rustpython_vm::builtins::PyList>().unwrap();
            let callbacks=events.borrow_vec().iter().map(|x|x.downcast_ref::<rustpython_vm::builtins::PyStr>().unwrap().as_utf8().unwrap().as_str().to_owned()).collect::<Vec<_>>();
            serde_json::json!({"ordered_callbacks":callbacks,"undo":self.undo.len(),"redo":self.redo.len()})
        })
    }
    pub fn observe_eq_outcome(&self)->serde_json::Value {
     self.interpreter.enter(|vm|{use rustpython_vm::{AsObject,builtins::PyList};
      let get=|k:&str|self.scope.globals.get_item(k,vm).unwrap();let e=get("e").downcast::<editable_mesh::Edge>().unwrap();let soft=e.soft.read().clone().unwrap();let hidden=e.hidden.read().clone().unwrap();let calls=get("eq_calls").downcast::<PyList>().unwrap();
      serde_json::json!({"calls":calls.borrow_vec().len(),"undo":self.undo.len(),"redo":self.redo.len(),"payload_is_old":soft.is(&get("oldp")),"payload_is_new":soft.is(&get("newp")),"hidden":hidden.try_to_bool(vm).unwrap()})
     })
    }
    pub fn observe_graph(&self,name:&str)->serde_json::Value {
      self.interpreter.enter(|vm|{
       use rustpython_vm::{AsObject,builtins::{PyList,PySet}};
       let get=|key:&str|self.scope.globals.get_item(key,vm).unwrap();
       let mesh=get("m").downcast::<editable_mesh::Mesh>().unwrap();
       let vertices=get("V").downcast::<PyList>().unwrap();let vertices=vertices.borrow_vec();
       let edges=get("E").downcast::<PyList>().unwrap();let edges=edges.borrow_vec();
       let faces=get("F").downcast::<PyList>().unwrap();let faces=faces.borrow_vec();
       let id=|value:&rustpython_vm::PyObject,items:&[rustpython_vm::PyObjectRef]|items.iter().position(|item|item.is(value));
       let incidence=vertices.iter().map(|v|{let v=v.downcast_ref::<editable_mesh::Vertex>().unwrap();let set=v.edges.read().clone().unwrap().downcast::<PySet>().unwrap();let mut values=set.elements().iter().map(|e|id(e,&edges)).collect::<Vec<_>>();values.sort();values}).collect::<Vec<_>>();
       let radial=edges.iter().map(|e|{let e=e.downcast_ref::<editable_mesh::Edge>().unwrap();let list=e.faces.read().clone().unwrap().downcast::<PyList>().unwrap();let values=list.borrow_vec();values.iter().map(|f|id(f,&faces)).collect::<Vec<_>>()}).collect::<Vec<_>>();
       let endpoints=edges.iter().map(|e|{let e=e.downcast_ref::<editable_mesh::Edge>().unwrap();[id(e.v0.read().as_ref().unwrap().as_object(),&vertices),id(e.v1.read().as_ref().unwrap().as_object(),&vertices)]}).collect::<Vec<_>>();
       let v=get("v").downcast::<editable_mesh::Vertex>().unwrap();let e=get("e").downcast::<editable_mesh::Edge>().unwrap();
       serde_json::json!({"name":name,"vertices":mesh.vertices.borrow_vec().iter().map(|x|id(x,&vertices)).collect::<Vec<_>>(),"edges":mesh.edges.borrow_vec().iter().map(|x|id(x,&edges)).collect::<Vec<_>>(),"faces":mesh.faces.borrow_vec().iter().map(|x|id(x,&faces)).collect::<Vec<_>>(),"incidence":incidence,"radial":radial,"endpoints":endpoints,"persistent_lists":[mesh.vertices.is(&get("oldvertices")),mesh.edges.is(&get("oldedges")),mesh.faces.is(&get("oldfaces"))],"held_incidence_original":v.edges.read().as_ref().unwrap().is(&get("oldvedges")),"held_radial_original":e.faces.read().as_ref().unwrap().is(&get("oldefaces")),"soft_tag_identity":e.soft.read().as_ref().unwrap().is(&get("tag")),"layer_tag_identity":e.layer.read().as_ref().unwrap().is(&get("tag")),"curve_tag_identity":e.curve.read().as_ref().unwrap().is(&get("tag")),"undo":self.undo.len(),"redo":self.redo.len()})
      })
    }
    pub fn native_counts(&self, name: &str) -> (usize, usize, usize) {
        self.interpreter.enter(|vm| {
            let mesh = self
                .scope
                .globals
                .get_item(name, vm)
                .unwrap()
                .downcast::<editable_mesh::Mesh>()
                .unwrap();
            let owner = mesh.owner.read();
            (
                owner.mesh().vertices.len(),
                owner.mesh().edges.len(),
                owner.mesh().faces.len(),
            )
        })
    }
}

impl Drop for Controller {
    fn drop(&mut self) {
        self.interpreter.enter(|vm| {
            vm.host_budget_abort();
            self.undo = Vec::new();
            self.redo = Vec::new();
            self.undo_slots.resize(0).expect("History slot release");
            self.redo_slots.resize(0).expect("History slot release");
            self.scope.globals.clear();
            vm.state.gc.collect_force(2);
        });
    }
}
