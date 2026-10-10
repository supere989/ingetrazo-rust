//! Real VM object graph plus native owned geometry; standalone prerequisite.
use crate::{
    face_owner::FaceOwner,
    owner_budget::{Kind, Lease, OwnerBudget},
    session,
};
use rustpython_vm::{
    Interpreter, InterpreterBuilder, PyRef, compiler::Mode, pymodule, scope::Scope,
};
use std::{cell::RefCell, rc::Rc};
struct Stage {
    groups: Vec<PyRef<native_groups::Group>>,
    charge: Lease,
    failed: bool,
    limit: Option<usize>,
}
struct Command {
    groups: Vec<PyRef<native_groups::Group>>,
    _charge: Lease,
}
struct State {
    budget: OwnerBudget,
    undo: Vec<Command>,
    redo: Vec<Command>,
}
static GROUP_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
thread_local! {static STAGED:RefCell<Option<Stage>>=const{RefCell::new(None)};static ACTIVE:RefCell<Option<Rc<RefCell<State>>>>=const{RefCell::new(None)};}
struct Guard;
impl Drop for Guard {
    fn drop(&mut self) {
        ACTIVE.with(|s| drop(s.borrow_mut().take()));
        STAGED.with(|s| drop(s.borrow_mut().take()));
    }
}
fn abort() {
    STAGED.with(|s| {
        if let Some(t) = s.borrow_mut().as_mut() {
            t.failed = true;
        }
    });
}
#[pymodule]
pub mod native_groups {
    use glam::DVec3;
    use rustpython_vm::types::{Constructor,AsNumber,Comparable};
    use ingetrazo_core::mesh::Mesh as NativeMesh;
    use rustpython_vm::{
        Py, PyObjectRef, PyPayload, PyRef, PyResult, Traverse, VirtualMachine, builtins::PyListRef,
        common::lock::PyRwLock, function::ArgIntoFloat, pyclass,
    };
    #[pyattr]
    #[pyclass(name = "QVector3D")]
    #[derive(Debug, PyPayload)]
    pub struct QVector3D { xyz: PyRwLock<[f32; 3]> }
    impl rustpython_vm::types::Constructor for QVector3D {
        type Args = rustpython_vm::function::PosArgs<PyObjectRef>;
        fn py_new(_cls: &Py<rustpython_vm::builtins::PyType>, args: Self::Args, vm: &VirtualMachine) -> PyResult<Self> {
            vm.host_budget_tick()?;
            let args=args.into_vec();
            let xyz=match args.as_slice() {
                [] => [0.;3],
                [v] => v.downcast_ref::<Self>().ok_or_else(||vm.new_type_error("QVector3D copy requires QVector3D"))?.xyz.read().to_owned(),
                [x,y,z] => [x.try_float(vm)?.to_f64() as f32,y.try_float(vm)?.to_f64() as f32,z.try_float(vm)?.to_f64() as f32],
                _ => return Err(vm.new_type_error("QVector3D requires zero, one vector, or three numeric arguments"))
            };
            Ok(Self{xyz:PyRwLock::new(xyz)})
        }
    }
    #[pyclass(with(Constructor, AsNumber, Comparable))]
    impl QVector3D {
        #[pymethod] fn x(zelf:&Py<Self>)->f64 {zelf.xyz.read()[0] as f64}
        #[pymethod] fn y(zelf:&Py<Self>)->f64 {zelf.xyz.read()[1] as f64}
        #[pymethod] fn z(zelf:&Py<Self>)->f64 {zelf.xyz.read()[2] as f64}
        #[pymethod(name="setX")] fn set_x(zelf:&Py<Self>,v:ArgIntoFloat,vm:&VirtualMachine)->PyResult<()> {vm.host_budget_tick()?;zelf.xyz.write()[0]=v.into_float() as f32;Ok(())}
        #[pymethod(name="setY")] fn set_y(zelf:&Py<Self>,v:ArgIntoFloat,vm:&VirtualMachine)->PyResult<()> {vm.host_budget_tick()?;zelf.xyz.write()[1]=v.into_float() as f32;Ok(())}
        #[pymethod(name="setZ")] fn set_z(zelf:&Py<Self>,v:ArgIntoFloat,vm:&VirtualMachine)->PyResult<()> {vm.host_budget_tick()?;zelf.xyz.write()[2]=v.into_float() as f32;Ok(())}
        #[pymethod(name="isNull")] fn is_null(zelf:&Py<Self>)->bool {*zelf.xyz.read()==[0.;3]}
        #[pymethod] fn normalized(zelf:&Py<Self>,vm:&VirtualMachine)->PyResult<Self> {vm.host_budget_tick()?;let a=*zelf.xyz.read();let n=Self::norm(a);let xyz=if (n-1.).abs()<=0.00001{a}else if n.abs()<=0.00001{[0.;3]}else{a.map(|v|v/n)};Ok(Self{xyz:PyRwLock::new(xyz)})}
        #[pymethod] fn normalize(zelf:&Py<Self>,vm:&VirtualMachine)->PyResult<()> {vm.host_budget_tick()?;let mut a=zelf.xyz.write();let n=Self::norm(*a);if (n-1.).abs()>0.00001 && n.abs()>0.00001 {*a=a.map(|v|v/n)};Ok(())}
        #[pystaticmethod(name="dotProduct")] fn dot(a:PyRef<Self>,b:PyRef<Self>,vm:&VirtualMachine)->PyResult<f64>{vm.host_budget_tick()?;let x=*a.xyz.read();let y=*b.xyz.read();Ok((x[0]*y[0]+x[1]*y[1]+x[2]*y[2]) as f64)}
        #[pystaticmethod(name="crossProduct")] fn cross(a:PyRef<Self>,b:PyRef<Self>,vm:&VirtualMachine)->PyResult<Self>{vm.host_budget_tick()?;let x=*a.xyz.read();let y=*b.xyz.read();Ok(Self{xyz:PyRwLock::new([x[1]*y[2]-x[2]*y[1],x[2]*y[0]-x[0]*y[2],x[0]*y[1]-x[1]*y[0]])})}
        #[pymethod(name="lengthSquared")] fn length_squared(zelf:&Py<Self>)->f64 {let a=*zelf.xyz.read();(a[0]*a[0]+a[1]*a[1]+a[2]*a[2]) as f64}
        #[pymethod] fn length(zelf:&Py<Self>)->f64 {Self::norm(*zelf.xyz.read()) as f64}
    }
    impl QVector3D {
        pub(crate) fn magnitude(xyz:[f32;3])->f32{Self::norm(xyz)}
        pub(crate) fn coordinates(&self)->[f32;3]{*self.xyz.read()}
        pub(crate) fn from_coordinates(xyz:[f32;3])->Self{Self{xyz:PyRwLock::new(xyz)}}
        fn norm(a:[f32;3])->f32 {let x=a[0].abs();let y=a[1].abs();let z=a[2].abs();let m=if x<y{if y<z{z}else{y}}else if x<z{z}else{x};if m==0.{0.}else{m*((x/m)*(x/m)+(y/m)*(y/m)+(z/m)*(z/m)).sqrt()}}
        fn inplace(a:&rustpython_vm::PyObject,b:&rustpython_vm::PyObject,vm:&VirtualMachine,op:u8)->PyResult {
            let Some(xyz)=Self::components(a,b,vm,op)? else{return Ok(vm.ctx.not_implemented())};
            let Some(v)=a.downcast_ref::<Self>()else{return Ok(vm.ctx.not_implemented())};
            *v.xyz.write()=xyz;Ok(a.to_owned())
        }
        fn binary(a:&rustpython_vm::PyObject,b:&rustpython_vm::PyObject,vm:&VirtualMachine,op:u8)->PyResult {
            let Some(xyz)=Self::components(a,b,vm,op)? else{return Ok(vm.ctx.not_implemented())};
            Ok(Self{xyz:PyRwLock::new(xyz)}.into_ref(&vm.ctx).into())
        }
        fn components(a:&rustpython_vm::PyObject,b:&rustpython_vm::PyObject,vm:&VirtualMachine,op:u8)->PyResult<Option<[f32;3]>> {
            vm.host_budget_tick()?;
            let (v,w)=if op==2 && a.downcast_ref::<Self>().is_none() {(b,a)} else {(a,b)};
            let Some(v)=v.downcast_ref::<Self>() else{return Ok(None)};
            let x=*v.xyz.read();
            let xyz=if op<2 {
                let Some(w)=w.downcast_ref::<Self>() else{return Ok(None)};let y=*w.xyz.read();
                std::array::from_fn(|i|if op==0{x[i]+y[i]}else{x[i]-y[i]})
            }else{let y=if let Some(w)=w.downcast_ref::<Self>(){*w.xyz.read()}else{[w.try_float(vm)?.to_f64() as f32;3]};std::array::from_fn(|i|if op==2{x[i]*y[i]}else{x[i]/y[i]})};
            Ok(Some(xyz))
        }
    }
    impl Comparable for QVector3D {
        fn cmp(zelf:&Py<Self>,other:&rustpython_vm::PyObject,op:rustpython_vm::types::PyComparisonOp,_vm:&VirtualMachine)->PyResult<rustpython_vm::function::PyComparisonValue>{
            op.eq_only(||Ok(match other.downcast_ref::<Self>(){Some(v)=>rustpython_vm::function::PyComparisonValue::Implemented(*zelf.xyz.read()==*v.xyz.read()),None=>rustpython_vm::function::PyComparisonValue::NotImplemented}))
        }
    }
    impl rustpython_vm::types::AsNumber for QVector3D {
        fn as_number()-> &'static rustpython_vm::protocol::PyNumberMethods {
            static METHODS:rustpython_vm::protocol::PyNumberMethods=rustpython_vm::protocol::PyNumberMethods {
                add:Some(|a,b,vm|QVector3D::binary(a,b,vm,0)),
                subtract:Some(|a,b,vm|QVector3D::binary(a,b,vm,1)),
                multiply:Some(|a,b,vm|QVector3D::binary(a,b,vm,2)),
                inplace_add:Some(|a,b,vm|QVector3D::inplace(a,b,vm,0)),
                inplace_subtract:Some(|a,b,vm|QVector3D::inplace(a,b,vm,1)),
                inplace_multiply:Some(|a,b,vm|QVector3D::inplace(a,b,vm,2)),
                inplace_true_divide:Some(|a,b,vm|QVector3D::inplace(a,b,vm,3)),
                negative:Some(|v,vm|{vm.host_budget_tick()?;let a=v.downcast_ref::<QVector3D>().unwrap();Ok(QVector3D{xyz:PyRwLock::new(a.xyz.read().map(|x|-x))}.into_ref(&vm.ctx).into())}),
                true_divide:Some(|a,b,vm|QVector3D::binary(a,b,vm,3)),
                ..rustpython_vm::protocol::PyNumberMethods::NOT_IMPLEMENTED
            }; &METHODS
        }
    }
    #[pyattr]
    #[pyclass(name = "Face", traverse)]
    #[derive(Debug, PyPayload)]
    pub struct Face {
        pub(super) attrs: PyRwLock<PyObjectRef>,
    }
    #[pyclass]
    impl Face {
        #[pygetset]
        fn attrs(zelf: &Py<Self>) -> PyObjectRef {
            zelf.attrs.read().clone()
        }
        #[pygetset(setter)]
        fn set_attrs(zelf: &Py<Self>, value: PyObjectRef, vm: &VirtualMachine) -> PyResult<()> {
            vm.host_budget_tick()?;
            let old = {
                let mut a = zelf.attrs.write();
                std::mem::replace(&mut *a, value)
            };
            drop(old);
            Ok(())
        }
    }
    #[pyattr]
    #[pyclass(name = "Mesh", traverse)]
    #[derive(Debug, PyPayload)]
    pub struct Mesh {
        #[pytraverse(skip)]
        pub(super) native: PyRwLock<NativeMesh>,
        pub(super) faces: PyListRef,
        #[pytraverse(skip)]
        pub(super) charge: PyRwLock<super::Lease>,
    }
    #[pyclass]
    impl Mesh {
        #[pygetset]
        fn faces(zelf: &Py<Self>) -> PyListRef {
            zelf.faces.clone()
        }
    }
    #[pyattr]
    #[pyclass(name = "Group", traverse)]
    #[derive(Debug, PyPayload)]
    pub struct Group {
        pub(super) mesh: PyRef<Mesh>,
        pub(super) name: PyObjectRef,
    }
    #[pyclass]
    impl Group {
        #[pygetset]
        fn mesh(zelf: &Py<Self>) -> PyRef<Mesh> {
            zelf.mesh.clone()
        }
        #[pygetset]
        fn name(zelf: &Py<Self>) -> PyObjectRef {
            zelf.name.clone()
        }
    }
    #[pyfunction]
    fn abort_stage() {
        super::abort();
    }
    /// Qt QVector3D construction boundary, not a substitute QVector3D class.
    #[pyfunction]
    fn point(
        x: ArgIntoFloat,
        y: ArgIntoFloat,
        z: ArgIntoFloat,
        vm: &VirtualMachine,
    ) -> PyResult<PyObjectRef> {
        vm.host_budget_tick()?;
        Ok(vm
            .ctx
            .new_tuple(vec![
                vm.ctx.new_float(x.into_float() as f32 as f64).into(),
                vm.ctx.new_float(y.into_float() as f32 as f64).into(),
                vm.ctx.new_float(z.into_float() as f32 as f64).into(),
            ])
            .into())
    }
    #[pyfunction]
    fn mesh_from_rings(rings: PyObjectRef, vm: &VirtualMachine) -> PyResult<PyRef<Mesh>> {
        vm.host_budget_tick()?;
        if super::STAGED.with(|s| s.borrow().as_ref().is_none_or(|t| t.failed)) {
            return Err(vm.new_value_error("Membership transaction refused"));
        }
        let result = (|| {
            let rows = vm.extract_elements_with(&rings, |row| {
                vm.extract_elements_with(&row, |p| {
                    let xyz = vm.extract_elements_with(&p, |x| Ok(x.try_float(vm)?.to_f64()))?;
                    if xyz.len() != 3 {
                        return Err(vm.new_value_error("Expected three coordinates"));
                    }
                    Ok(DVec3::new(xyz[0], xyz[1], xyz[2]))
                })
            })?;
            let budget = super::ACTIVE
                .with(|slot| slot.borrow().as_ref().map(|s| s.borrow().budget.clone()))
                .ok_or_else(|| vm.new_value_error("No active native owner"))?;
            let mut owner = super::FaceOwner::new_with_budget(NativeMesh::new(), budget.clone())
                .map_err(|e| vm.new_value_error(e))?;
            let mut t = owner.begin().map_err(|e| vm.new_value_error(e))?;
            let mut faces = Vec::new();
            faces
                .try_reserve_exact(rows.len())
                .map_err(|_| vm.new_value_error("Face handle reservation refused"))?;
            for row in &rows {
                vm.host_budget_tick()?;
                t.add_face(row, &[]).map_err(|e| vm.new_value_error(e))?;
                faces.push(
                    Face {
                        attrs: PyRwLock::new(vm.ctx.new_dict().into()),
                    }
                    .into_ref(&vm.ctx),
                );
            }
            owner.commit(t).map_err(|e| vm.new_value_error(e))?;
            let native = owner.into_mesh();
            let objects: Vec<PyObjectRef> = faces.into_iter().map(Into::into).collect();
            let bytes =
                mesh_bytes(&native, objects.capacity()).map_err(|e| vm.new_value_error(e))?;
            let charge = budget
                .reserve(super::Kind::Staging, bytes)
                .map_err(|e| vm.new_value_error(e))?;
            Ok(Mesh {
                native: PyRwLock::new(native),
                faces: vm.ctx.new_list(objects),
                charge: PyRwLock::new(charge),
            }
            .into_ref(&vm.ctx))
        })();
        if result.is_err() {
            super::abort();
        }
        result
    }
    fn mesh_bytes(m: &NativeMesh, face_handles: usize) -> Result<usize, String> {
        let mut n = face_handles
            .checked_mul(std::mem::size_of::<PyRef<Face>>())
            .ok_or("Native mesh handle extent overflow")?;
        for (capacity, size) in [
            (
                m.vertices.capacity(),
                std::mem::size_of::<ingetrazo_core::mesh::MeshVertex>(),
            ),
            (
                m.edges.capacity(),
                std::mem::size_of::<ingetrazo_core::mesh::MeshEdge>(),
            ),
            (
                m.faces.capacity(),
                std::mem::size_of::<ingetrazo_core::mesh::MeshFace>(),
            ),
        ] {
            n = n
                .checked_add(
                    capacity
                        .checked_mul(size)
                        .ok_or("Native mesh extent overflow")?,
                )
                .ok_or("Native mesh extent overflow")?;
        }
        for v in &m.vertices {
            n = n
                .checked_add(
                    v.edges
                        .capacity()
                        .checked_mul(std::mem::size_of::<usize>())
                        .ok_or("Native adjacency extent overflow")?,
                )
                .ok_or("Native adjacency extent overflow")?;
        }
        for e in &m.edges {
            n = n
                .checked_add(
                    e.faces
                        .capacity()
                        .checked_mul(std::mem::size_of::<usize>())
                        .ok_or("Native radial extent overflow")?,
                )
                .ok_or("Native radial extent overflow")?;
        }
        for e in &m.edges {
            if !e.attrs.is_empty() || e.attrs.capacity() != 0 || e.layer.is_some() {
                return Err("Native edge metadata payload not admitted by this builder".into());
            }
        }
        for f in &m.faces {
            if !f.attrs.is_empty() || f.attrs.capacity() != 0 || f.layer.is_some() {
                return Err("Native face metadata payload not admitted by this builder".into());
            }
            n = n
                .checked_add(
                    f.vertices
                        .capacity()
                        .checked_mul(std::mem::size_of::<usize>())
                        .ok_or("Native ring extent overflow")?,
                )
                .and_then(|n| {
                    n.checked_add(
                        f.holes
                            .capacity()
                            .checked_mul(std::mem::size_of::<Vec<usize>>())?,
                    )
                })
                .ok_or("Native hole extent overflow")?;
            for h in &f.holes {
                n = n
                    .checked_add(
                        h.capacity()
                            .checked_mul(std::mem::size_of::<usize>())
                            .ok_or("Native hole ring extent overflow")?,
                    )
                    .ok_or("Native hole ring extent overflow")?;
            }
        }
        n = n
            .checked_add(
                m.spatial_index_capacity()
                    .checked_mul(std::mem::size_of::<(ingetrazo_core::math::PointKey, usize)>())
                    .ok_or("Native index extent overflow")?,
            )
            .ok_or("Native index extent overflow")?;
        Ok(n)
    }
    #[pyfunction]
    fn orient_and_soften(mesh: PyRef<Mesh>, vm: &VirtualMachine) -> PyResult<()> {
        vm.host_budget_tick()?;
        let result = (|| {
            let mut m = mesh.native.write();
            if !m.faces.is_empty() {
                if source_coplanar(&m) {
                    for f in &mut m.faces {
                        f.interior = false;
                    }
                } else {
                    ingetrazo_core::heal::orient_boundary_candidate(&mut m)
                        .map_err(|e| vm.new_value_error(e))?;
                }
            }
            let normals = (0..m.faces.len())
                .map(|i| source_normal(&m, i))
                .collect::<Vec<_>>();
            for e in &mut m.edges {
                if e.faces.len() == 2 {
                    let a = normals[e.faces[0]];
                    let b = normals[e.faces[1]];
                    let dot = a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
                    if (dot as f64) > 42_f64.to_radians().cos() {
                        e.soft = true;
                    }
                }
            }
            Ok(())
        })();
        if result.is_err() {
            super::abort();
        }
        result
    }
    fn source_normal(m: &NativeMesh, i: usize) -> [f32; 3] {
        let ids = &m.faces[i].vertices;
        if ids.len() < 3 {
            return [0., 0., 1.];
        }
        let mut n = [0_f64; 3];
        for j in 0..ids.len() {
            let a = m.vertices[ids[j]].position;
            let b = m.vertices[ids[(j + 1) % ids.len()]].position;
            n[0] += (a.y - b.y) * (a.z + b.z);
            n[1] += (a.z - b.z) * (a.x + b.x);
            n[2] += (a.x - b.x) * (a.y + b.y);
        }
        let n = n.map(|x| x as f32);
        let length = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        if (length as f64) < 1e-9 {
            return [0., 0., 1.];
        }
        n.map(|x| x / length)
    }
    fn centroid(m: &NativeMesh, i: usize) -> [f32; 3] {
        let ids = &m.faces[i].vertices;
        let mut v = [0_f64; 3];
        for &id in ids {
            let p = m.vertices[id].position;
            v[0] += p.x;
            v[1] += p.y;
            v[2] += p.z;
        }
        if !ids.is_empty() {
            v = v.map(|x| x / ids.len() as f64);
        }
        v.map(|x| x as f32)
    }
    fn source_coplanar(m: &NativeMesh) -> bool {
        let n = source_normal(m, 0);
        let o = centroid(m, 0);
        (0..m.faces.len()).all(|i| {
            let a = source_normal(m, i);
            let c = centroid(m, i);
            let dot = a[0] * n[0] + a[1] * n[1] + a[2] * n[2];
            let dist = (c[0] - o[0]) * n[0] + (c[1] - o[1]) * n[1] + (c[2] - o[2]) * n[2];
            !((dot.abs() as f64) < 0.999) && !((dist.abs() as f64) > 1e-4)
        })
    }
    #[pyfunction]
    fn auto_name(vm: &VirtualMachine) -> PyResult<String> {
        vm.host_budget_tick()?;
        if !super::ACTIVE.with(|slot| slot.borrow().is_some()) {
            return Err(vm.new_runtime_error("No active owner"));
        }
        let n = super::GROUP_COUNTER
            .fetch_update(
                std::sync::atomic::Ordering::Relaxed,
                std::sync::atomic::Ordering::Relaxed,
                |n| n.checked_add(1),
            )
            .map_err(|_| vm.new_value_error("Group name counter exhausted"))?;
        Ok(format!("Group {n}"))
    }
    #[pyfunction]
    fn group(mesh: PyRef<Mesh>, name: PyObjectRef, vm: &VirtualMachine) -> PyResult<PyRef<Group>> {
        vm.host_budget_tick()?;
        Ok(Group { mesh, name }.into_ref(&vm.ctx))
    }
    #[pyfunction]
    fn append(group: PyRef<Group>, vm: &VirtualMachine) -> PyResult<()> {
        vm.host_budget_tick()?;
        let result = super::STAGED.with(|slot| {
            let mut slot = slot.borrow_mut();
            let t = slot.as_mut().ok_or("No active membership transaction")?;
            if t.failed {
                return Err("Membership transaction refused".into());
            }
            let count = t
                .groups
                .len()
                .checked_add(1)
                .ok_or("Membership extent overflow")?;
            if t.limit.is_some_and(|limit| count > limit) {
                return Err("Group handle admission denied".into());
            }
            t.groups
                .try_reserve(1)
                .map_err(|_| "Group handle slots refused")?;
            t.charge.resize(
                t.groups
                    .capacity()
                    .checked_mul(std::mem::size_of::<PyRef<Group>>())
                    .ok_or("Group charge overflow")?,
            )?;
            t.groups.push(group);
            Ok::<_, String>(())
        });
        if result.is_err() {
            super::abort();
        }
        result.map_err(|e| vm.new_value_error(e))
    }
    #[pyfunction]
    fn groups(vm: &VirtualMachine) -> PyResult<PyListRef> {
        vm.host_budget_tick()?;
        let mut groups = super::ACTIVE
            .with(|s| {
                let s = s.borrow();
                let s = s.as_ref().ok_or("No active owner")?.borrow();
                let mut out = Vec::new();
                let count = s.undo.iter().map(|c| c.groups.len()).sum();
                out.try_reserve_exact(count)
                    .map_err(|_| "Membership query refused")?;
                for c in &s.undo {
                    out.extend(c.groups.iter().cloned().map(Into::into));
                }
                Ok::<_, String>(out)
            })
            .map_err(|e| vm.new_value_error(e))?;
        super::STAGED.with(|s| {
            if let Some(t) = s.borrow().as_ref() {
                groups
                    .try_reserve(t.groups.len())
                    .map_err(|_| vm.new_value_error("Staged membership query refused"))?;
                groups.extend(t.groups.iter().cloned().map(Into::into));
            }
            Ok::<_, rustpython_vm::builtins::PyBaseExceptionRef>(())
        })?;
        Ok(vm.ctx.new_list(groups))
    }
}
const SETUP: &str = r#"import native_groups
Mesh=native_groups.Mesh
Group=native_groups.Group
Face=native_groups.Face
QVector3D=native_groups.QVector3D
def _finish(mesh,color,name):
    if color is not None:
        for f in mesh.faces:
            f.attrs['color']=tuple(color)
    native_groups.orient_and_soften(mesh)
    group=native_groups.group(mesh,name or native_groups.auto_name())
    native_groups.append(group)
    return group
def extrude(outline,z0,z1,name=None,color=None):
    lo=[native_groups.point(x,y,z0) for x,y in outline]
    hi=[native_groups.point(x,y,z1) for x,y in outline]
    rings=[]
    n=len(lo)
    for i in range(n):
        j=(i+1)%n
        rings.append([lo[i],lo[j],hi[j],hi[i]])
    rings.append(list(reversed(lo)))
    rings.append(hi)
    return _finish(native_groups.mesh_from_rings(rings),color,name)
"#;
#[derive(Debug)]
pub enum Failure {
    Python(rustpython_vm::builtins::PyBaseExceptionRef),
    Preparation(String),
}
pub struct Controller {
    scope: Scope,
    state: Rc<RefCell<State>>,
    interpreter: Rc<Interpreter>,
    tainted: bool,
    limit: Option<usize>,
}
impl Controller {
    pub fn print_failure(&self,f:&Failure){match f {Failure::Python(e)=>self.interpreter.enter(|vm|vm.print_exception(e)),Failure::Preparation(e)=>eprintln!("{e}")}}
    pub fn new() -> Self {
        let builder = InterpreterBuilder::new();
        let module = native_groups::module_def(&builder.ctx);
        let interpreter = builder.add_native_module(module).interpreter();
        let scope = interpreter.enter(|vm| {
            let helpers = vm.new_scope_with_builtins();
            vm.run_code_obj(
                vm.compile(SETUP, Mode::Exec, "<setup>").unwrap(),
                helpers.clone(),
            )
            .unwrap();
            let scope = vm.new_scope_with_builtins();
            for name in [
                "native_groups",
                "Mesh",
                "Group",
                "Face",
                "QVector3D",
                "extrude",
                "_finish",
            ] {
                scope
                    .globals
                    .set_item(name, helpers.globals.get_item(name, vm).unwrap(), vm)
                    .unwrap();
            }
            scope
        });
        Self {
            scope,
            state: Rc::new(RefCell::new(State {
                budget: OwnerBudget::new(None),
                undo: vec![],
                redo: vec![],
            })),
            interpreter: Rc::new(interpreter),
            tainted: false,
            limit: None,
        }
    }
    pub fn execute(&mut self, source: &str, fuel: u64) -> Result<bool, Failure> {
        if self.tainted
            || STAGED.with(|s| s.borrow().is_some())
            || ACTIVE.with(|s| s.borrow().is_some())
        {
            return Err(Failure::Preparation("VM admission refused".into()));
        }
        let stage = Stage {
            groups: vec![],
            charge: self
                .state
                .borrow()
                .budget
                .reserve(Kind::Staging, 0)
                .map_err(Failure::Preparation)?,
            failed: false,
            limit: self.limit,
        };
        let state = self.state.clone();
        let scope = self.scope.clone();
        self.tainted = true;
        let result = self.interpreter.enter(|vm| {
            vm.host_budget_arm(fuel)
                .map_err(|e| Failure::Preparation(e.to_owned()))?;
            STAGED.with(|s| *s.borrow_mut() = Some(stage));
            ACTIVE.with(|s| *s.borrow_mut() = Some(state.clone()));
            let _guard = Guard;
            let result = session::execute(vm, &scope, source);
            let poison = vm.host_budget_poisoned();
            ACTIVE.with(|s| drop(s.borrow_mut().take()));
            let mut t = STAGED.with(|s| s.borrow_mut().take()).unwrap();
            let result = match result {
                Err(e) => {
                    drop(t);
                    Err(Failure::Python(e))
                }
                Ok(()) if poison || t.failed => {
                    drop(t);
                    Err(Failure::Preparation(
                        "Membership transaction refused".into(),
                    ))
                }
                Ok(()) if t.groups.is_empty() => {
                    drop(t);
                    Ok(false)
                }
                Ok(()) => {
                    let mut s = state.borrow_mut();
                    let result = (|| {
                        s.undo
                            .try_reserve(1)
                            .map_err(|_| "Group command history refused")?;
                        t.charge.resize(
                            t.groups
                                .capacity()
                                .checked_mul(std::mem::size_of::<PyRef<native_groups::Group>>())
                                .and_then(|n| n.checked_add(std::mem::size_of::<Command>()))
                                .ok_or("Group history overflow")?,
                        )?;
                        t.charge.reclassify(Kind::History)?;
                        for group in &t.groups {
                            group.mesh.charge.write().reclassify(Kind::History)?;
                        }
                        s.redo.clear();
                        s.undo.push(Command {
                            groups: t.groups,
                            _charge: t.charge,
                        });
                        Ok::<_, String>(true)
                    })();
                    result.map_err(Failure::Preparation)
                }
            };
            vm.host_budget_reset_after_discard();
            result
        });
        self.tainted = false;
        result
    }
    pub fn undo(&mut self) -> Result<bool, String> {
        let mut s = self.state.borrow_mut();
        if s.undo.is_empty() {
            return Ok(false);
        }
        s.redo
            .try_reserve(1)
            .map_err(|_| "Group redo history refused")?;
        let c = s.undo.pop().unwrap();
        s.redo.push(c);
        Ok(true)
    }
    pub fn redo(&mut self) -> Result<bool, String> {
        let mut s = self.state.borrow_mut();
        if s.redo.is_empty() {
            return Ok(false);
        }
        s.undo
            .try_reserve(1)
            .map_err(|_| "Group undo history refused")?;
        let c = s.redo.pop().unwrap();
        s.undo.push(c);
        Ok(true)
    }
    pub fn hold_group(&self, name: &str) -> HeldGroup {
        let group = self.interpreter.enter(|vm| {
            self.scope
                .globals
                .get_item(name, vm)
                .unwrap()
                .downcast::<native_groups::Group>()
                .unwrap()
        });
        HeldGroup {
            group: Some(group),
            interpreter: self.interpreter.clone(),
        }
    }
    pub fn observe_geometry(&self, name: &str) -> serde_json::Value {
        self.interpreter.enter(|vm|{let g=self.scope.globals.get_item(name,vm).unwrap().downcast::<native_groups::Group>().unwrap();let m=g.mesh.native.read();serde_json::json!({"vertices":m.vertices.iter().map(|v|v.position.to_array()).collect::<Vec<_>>(),"edges":m.edges.iter().map(|e|serde_json::json!({"vertices":[e.v0,e.v1],"soft":e.soft})).collect::<Vec<_>>(),"faces":m.faces.iter().map(|f|serde_json::json!({"vertices":f.vertices,"interior":f.interior})).collect::<Vec<_>>()})})
    }
    pub fn set_limit(&mut self, limit: Option<usize>) {
        self.limit = limit;
    }
    pub fn budget(&self) -> OwnerBudget {
        self.state.borrow().budget.clone()
    }
    pub fn counts(&self) -> (usize, usize, usize) {
        let s = self.state.borrow();
        (
            s.undo.iter().map(|c| c.groups.len()).sum(),
            s.undo.len(),
            s.redo.len(),
        )
    }
    pub fn observe(&self, name: &str) -> serde_json::Value {
        use rustpython_vm::{AsObject, builtins::PyDict};
        self.interpreter.enter(|vm|{let g=self.scope.globals.get_item("g",vm).unwrap().downcast::<native_groups::Group>().unwrap();let held=self.scope.globals.get_item("held_mesh",vm).unwrap();let face=self.scope.globals.get_item("held_face",vm).unwrap();let s=self.state.borrow();let live:Vec<_>=s.undo.iter().flat_map(|c|c.groups.iter()).collect();let m=g.mesh.native.read();let face_items=g.mesh.faces.borrow_vec();let mut types=face_items.iter().map(|f|f.downcast_ref::<native_groups::Face>().unwrap().attrs.read().clone().downcast::<PyDict>().unwrap().get_item("color",vm).unwrap().class().name().to_string()).collect::<Vec<_>>();types.sort();types.dedup();serde_json::json!({"name":name,"groups":live.len(),"returned_is_placement":live.iter().any(|x|x.is(&g)),"held_mesh_is_group":g.mesh.is(&held),"held_face_is_mesh_first":face_items.first().is_some_and(|x|x.is(&face)),"held_mesh_faces":m.faces.len(),"soft_edges":m.edges.iter().filter(|e|e.soft).count(),"face_color_types":types})})
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
            {
                let mut s = self.state.borrow_mut();
                s.undo.clear();
                s.redo.clear();
            }
            vm.state.gc.collect_force(2);
        });
    }
}

/// Opaque external reference keeps the real VM and native Mesh lease alive.
/// Closed controller sessions cannot execute new code through this observer.
pub struct HeldGroup {
    group: Option<PyRef<native_groups::Group>>,
    interpreter: Rc<Interpreter>,
}
impl HeldGroup {
    pub fn literal_name(&self) -> String {
        self.group
            .as_ref()
            .unwrap()
            .name
            .downcast_ref::<rustpython_vm::builtins::PyStr>()
            .unwrap()
            .as_wtf8()
            .to_string()
    }
    pub fn counts(&self) -> (usize, usize, usize) {
        let m = self.group.as_ref().unwrap().mesh.native.read();
        (m.vertices.len(), m.edges.len(), m.faces.len())
    }
    pub fn charged_bytes(&self) -> usize {
        self.group.as_ref().unwrap().mesh.charge.read().bytes()
    }
}
impl Drop for HeldGroup {
    fn drop(&mut self) {
        self.interpreter.enter(|vm| {
            drop(self.group.take());
            vm.state.gc.collect_force(2);
        });
    }
}
