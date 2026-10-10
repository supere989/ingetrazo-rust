//! Genuine editable VM Mesh prerequisite, append-only staged mutation.
use crate::{
    face_owner::{FaceOwner, FaceTransaction},
    owner_budget::{Kind, Lease, OwnerBudget},
    session,
};
use rustpython_vm::{
    Interpreter, InterpreterBuilder, PyRef, compiler::Mode, pymodule, scope::Scope,
};
use std::{cell::RefCell, collections::HashMap};
struct Entry {
    mesh: PyRef<editable_mesh::Mesh>,
    transaction: Option<FaceTransaction>,
    faces: usize,
    vertices: usize,
    published: bool,
}
impl Drop for Entry {
    fn drop(&mut self) {
        if !self.published {
            self.mesh.faces.borrow_vec_mut().truncate(self.faces);
            self.mesh.vertices.borrow_vec_mut().truncate(self.vertices);
        }
    }
}
struct Stage {
    entries: HashMap<usize, Entry>,
    budget: OwnerBudget,
    charge: Lease,
    failed: bool,
}
impl Stage {
    fn rollback(&mut self) {
        for entry in self.entries.values() {
            entry.mesh.faces.borrow_vec_mut().truncate(entry.faces);
            entry
                .mesh
                .vertices
                .borrow_vec_mut()
                .truncate(entry.vertices);
        }
    }
}
thread_local! {static STAGED:RefCell<Option<Stage>>=const{RefCell::new(None)};}
struct Guard;
impl Drop for Guard {
    fn drop(&mut self) {
        STAGED.with(|s| {
            if let Some(mut stage) = s.borrow_mut().take() {
                stage.rollback();
            }
        })
    }
}
#[pymodule]
pub mod editable_mesh {
    use super::*;
    use crate::vm_groups::native_groups::QVector3D;
    use rustpython_vm::{
        AsObject, Py, PyObjectRef, PyPayload, PyResult, Traverse, VirtualMachine,
        builtins::{PyListRef, PyType},
        common::lock::PyRwLock,
        function::{OptionalArg, PosArgs},
        pyclass,
        types::Constructor,
    };
    #[pyattr]
    #[pyclass(name = "Vertex", traverse)]
    #[derive(Debug, PyPayload)]
    pub struct Vertex {
        pub(super) position: PyRef<QVector3D>,
    }
    #[pyclass]
    impl Vertex {
        #[pygetset]
        fn position(zelf: &Py<Self>) -> PyRef<QVector3D> {
            zelf.position.clone()
        }
    }
    #[pyattr]
    #[pyclass(name = "Face", traverse)]
    #[derive(Debug, PyPayload)]
    pub struct Face {
        pub(super) r#loop: PyListRef,
        hole_loops: PyListRef,
        pub(super) attrs: PyRwLock<PyObjectRef>,
    }
    #[pyclass]
    impl Face {
        #[pygetset(name = "loop")]
        fn face_loop(zelf: &Py<Self>) -> PyListRef {
            zelf.r#loop.clone()
        }
        #[pygetset]
        fn hole_loops(zelf: &Py<Self>) -> PyListRef {
            zelf.hole_loops.clone()
        }
        #[pygetset]
        fn vertices(zelf: &Py<Self>, vm: &VirtualMachine) -> PyResult<PyListRef> {
            vm.host_budget_tick()?;
            let mut positions = Vec::new();
            let ring = zelf.r#loop.borrow_vec();
            positions
                .try_reserve_exact(ring.len())
                .map_err(|_| vm.new_value_error("Face position slots refused"))?;
            for vertex in ring.iter() {
                let vertex = vertex
                    .downcast_ref::<Vertex>()
                    .ok_or_else(|| vm.new_type_error("Face loop requires Vertex"))?;
                positions.push(vertex.position.clone().into());
            }
            Ok(vm.ctx.new_list(positions))
        }
        #[pymethod]
        fn normal(zelf: &Py<Self>, vm: &VirtualMachine) -> PyResult<PyRef<QVector3D>> {
            vm.host_budget_tick()?;
            let ring = zelf.r#loop.borrow_vec();
            let mut n = [0f64; 3];
            for i in 0..ring.len() {
                let p = ring[i]
                    .downcast_ref::<Vertex>()
                    .ok_or_else(|| vm.new_type_error("Face loop requires Vertex"))?
                    .position
                    .coordinates()
                    .map(f64::from);
                let q = ring[(i + 1) % ring.len()]
                    .downcast_ref::<Vertex>()
                    .ok_or_else(|| vm.new_type_error("Face loop requires Vertex"))?
                    .position
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
            let ring = zelf.r#loop.borrow_vec();
            let mut sum = [0f64; 3];
            for v in ring.iter() {
                let p = v
                    .downcast_ref::<Vertex>()
                    .ok_or_else(|| vm.new_type_error("Face loop requires Vertex"))?
                    .position
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
                .with(|s| s.borrow().as_ref().map(|s| s.budget.clone()))
                .ok_or_else(|| vm.new_value_error("Mesh requires active transaction"))?;
            let owner = FaceOwner::new_with_budget(ingetrazo_core::mesh::Mesh::new(), budget)
                .map_err(|e| vm.new_value_error(e))?;
            Ok(Self {
                owner: PyRwLock::new(owner),
                faces: vm.ctx.new_list(vec![]),
                vertices: vm.ctx.new_list(vec![]),
                managed: std::cell::Cell::new(false),
            })
        }
    }
    #[pyclass(with(Constructor))]
    impl Mesh {
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
                let mut slot = cell.borrow_mut();
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
                    vertices
                        .try_reserve(growth)
                        .map_err(|_| vm.new_value_error("Vertex list reservation refused"))?;
                    for p in &positions[vertices.len() - base..] {
                        let position =
                            QVector3D::from_coordinates(p.as_vec3().to_array()).into_ref(&vm.ctx);
                        vertices.push(Vertex { position }.into_ref(&vm.ctx).into());
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
                        r#loop,
                        hole_loops: vm.ctx.new_list(hole_lists),
                        attrs: PyRwLock::new(vm.ctx.new_dict().into()),
                    }
                    .into_ref(&vm.ctx);
                    let mut faces = zelf.faces.borrow_vec_mut();
                    faces
                        .try_reserve(1)
                        .map_err(|_| vm.new_value_error("Face list reservation refused"))?;
                    faces.push(face.clone().into());
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
pub struct Controller {
    interpreter: Interpreter,
    scope: Scope,
    budget: OwnerBudget,
    tainted: bool,
    pressure_after: Option<usize>,
    last_prepared: usize,
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
        let scope=interpreter.enter(|vm|{let s=vm.new_scope_with_builtins();vm.run_code_obj(vm.compile("from native_groups import QVector3D\nfrom editable_mesh import Mesh,Vertex,Face",Mode::Exec,"<setup>").unwrap(),s.clone()).unwrap();s});
        let mut controller = Self {
            interpreter,
            scope,
            budget: OwnerBudget::new(None),
            tainted: false,
            pressure_after: None,
            last_prepared: 0,
        };
        controller.execute("mesh=Mesh()").unwrap();
        controller.adopt("mesh");
        controller
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
            let _guard = Guard;
            vm.host_budget_arm(100000)
                .map_err(|_| "VM fuel admission refused")?;
            let executed = session::execute(vm, &self.scope, source);
            let mut stage = STAGED.with(|s| s.borrow_mut().take().unwrap());
            let result = if let Err(error) = executed {
                vm.print_exception(&error);
                stage.rollback();
                Err("Python execution refused".into())
            } else if vm.host_budget_poisoned() || stage.failed {
                stage.rollback();
                Err("Staged mutation refused".into())
            } else {
                let mut entries = Vec::new();
                entries
                    .try_reserve_exact(stage.entries.len())
                    .map_err(|_| "Publication entries refused")?;
                entries.extend(stage.entries.drain().map(|(_, e)| e));
                let publish = (|| {
                    let mut transactions = Vec::new();
                    transactions
                        .try_reserve_exact(entries.len())
                        .map_err(|_| "Publication transaction slots refused")?;
                    for entry in &mut entries {
                        transactions.push(entry.transaction.take().unwrap());
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
                    for (owner, transaction) in locks.iter_mut().zip(transactions) {
                        prepared.push(owner.prepare(transaction)?);
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
                    for e in &entries {
                        e.mesh.faces.borrow_vec_mut().truncate(e.faces);
                        e.mesh.vertices.borrow_vec_mut().truncate(e.vertices);
                    }
                } else {
                    for e in &mut entries {
                        e.published = true;
                    }
                }
                publish
            };
            vm.host_budget_reset_after_discard();
            result
        });
        self.budget.set_limit(prior_limit);
        self.tainted = false;
        result
    }
    pub fn assert_script(&mut self, code: &str) {
        if let Err(e) = self.execute(code) {
            panic!("{e}")
        }
    }
    pub fn observe(&self, name: &str) -> serde_json::Value {
        self.interpreter.enter(|vm|{use rustpython_vm::AsObject;let mesh=self.scope.globals.get_item("m",vm).unwrap().downcast::<editable_mesh::Mesh>().unwrap();let owner=mesh.owner.read();let m=owner.mesh();let held=self.scope.globals.get_item("held",vm).unwrap().downcast::<editable_mesh::Face>().unwrap();let old_loop=self.scope.globals.get_item("loop",vm).unwrap();let attrs=self.scope.globals.get_item("heldattrs",vm).unwrap();let position=self.scope.globals.get_item("heldposition",vm).unwrap();let live=mesh.faces.borrow_vec();let first=held.r#loop.borrow_vec()[0].clone().downcast::<editable_mesh::Vertex>().unwrap();serde_json::json!({"name":name,"mesh":{"vertices":m.vertices.iter().map(|v|v.position.to_array()).collect::<Vec<_>>(),"edges":m.edges.iter().map(|e|[e.v0,e.v1]).collect::<Vec<_>>(),"faces":m.faces.iter().map(|f|serde_json::json!({"vertices":f.vertices,"holes":f.holes})).collect::<Vec<_>>()},"persistent_faces":mesh.faces.is(&self.scope.globals.get_item("faces",vm).unwrap()),"persistent_vertices":mesh.vertices.is(&self.scope.globals.get_item("vertices",vm).unwrap()),"held_face_live":live.iter().any(|v|v.is(&held)),"face_loop_preserved":held.r#loop.is(&old_loop),"attrs_dict_preserved":held.attrs.read().is(&attrs),"vertex_position_preserved":first.position.is(&position)})})
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
            if !vm.host_budget_active() {
                let _ = vm.host_budget_arm(0);
            }
            let _ = vm.host_budget_tick();
            self.scope.globals.clear();
            vm.state.gc.collect_force(2);
        });
    }
}
