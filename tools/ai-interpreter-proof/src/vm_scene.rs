//! Persistent Scene/group aliases for the opt-in application console.
//! Classic group membership is command-owned; existing group contents are not.
use super::{editable_mesh, hydrate, packed, RootRestore};
use crate::owner_budget::{Kind, Lease, OwnerBudget};
use rustpython_vm::{builtins::PyListRef, AsObject, PyObjectRef, PyPayload, PyRef, VirtualMachine};
use std::sync::atomic::{AtomicU64, Ordering};
static GROUP_NUMBER: AtomicU64 = AtomicU64::new(1);

#[rustpython_vm::pymodule]
pub(super) mod console_scene {
    use super::*;
    use rustpython_vm::{
        builtins::{PyDictRef, PyType},
        common::lock::PyRwLock,
        pyclass,
        types::Constructor,
        FromArgs, Py, PyResult, Traverse,
    };
    #[pyattr]
    #[pyclass(name = "QMatrix4x4", traverse)]
    #[derive(Debug, PyPayload)]
    pub struct QMatrix4x4 {
        #[pytraverse(skip)]
        pub(super) values: PyRwLock<[f32; 16]>,
    }
    impl Constructor for QMatrix4x4 {
        type Args = rustpython_vm::function::PosArgs<PyObjectRef>;
        fn py_new(_cls: &Py<PyType>, args: Self::Args, vm: &VirtualMachine) -> PyResult<Self> {
            vm.host_budget_tick()?;
            let args = args.into_vec();
            let values = match args.as_slice() {
                [] => glam::Mat4::IDENTITY.to_cols_array(),
                [value] => *value
                    .downcast_ref::<Self>()
                    .ok_or_else(|| vm.new_type_error("QMatrix4x4 copy requires matrix"))?
                    .values
                    .read(),
                row if row.len() == 16 => {
                    let mut v = [0.; 16];
                    for r in 0..4 {
                        for c in 0..4 {
                            v[c * 4 + r] = row[r * 4 + c].try_float(vm)?.to_f64() as f32;
                        }
                    }
                    v
                }
                _ => {
                    return Err(vm.new_type_error(
                        "QMatrix4x4 requires zero, copy, or sixteen numeric arguments",
                    ))
                }
            };
            Ok(Self {
                values: PyRwLock::new(values),
            })
        }
    }
    impl QMatrix4x4 {
        pub(super) fn from_native(matrix: glam::DMat4) -> Result<Self, String> {
            let source = matrix.to_cols_array();
            let values = source.map(|v| v as f32);
            if source
                .iter()
                .zip(values)
                .any(|(&source, value)| !source.is_finite() || f64::from(value) != source)
            {
                return Err(
                    "Native Group placement cannot be represented exactly by QMatrix4x4".into(),
                );
            }
            Ok(Self {
                values: PyRwLock::new(values),
            })
        }
        pub(super) fn native(&self) -> Result<glam::DMat4, String> {
            let v = *self.values.read();
            if !v.iter().all(|v| v.is_finite())
                || v[3] != 0.
                || v[7] != 0.
                || v[11] != 0.
                || v[15] != 1.
            {
                return Err("Group placement requires finite affine QMatrix4x4".into());
            }
            Ok(glam::DMat4::from_cols_array(&v.map(f64::from)))
        }
        fn xyz(
            args: rustpython_vm::function::PosArgs<PyObjectRef>,
            uniform: bool,
            vm: &VirtualMachine,
        ) -> PyResult<[f32; 3]> {
            let args = args.into_vec();
            match args.as_slice() {
                [v] if v
                    .downcast_ref::<crate::vm_groups::native_groups::QVector3D>()
                    .is_some() =>
                {
                    Ok(
                        v.downcast_ref::<crate::vm_groups::native_groups::QVector3D>()
                            .unwrap()
                            .coordinates(),
                    )
                }
                [v] if uniform => Ok([v.try_float(vm)?.to_f64() as f32; 3]),
                [x, y, z] => Ok([
                    x.try_float(vm)?.to_f64() as f32,
                    y.try_float(vm)?.to_f64() as f32,
                    z.try_float(vm)?.to_f64() as f32,
                ]),
                _ => Err(vm.new_type_error("Matrix operation requires vector or three numbers")),
            }
        }
    }
    #[pyclass(with(Constructor))]
    impl QMatrix4x4 {
        #[pymethod(name = "isIdentity")]
        fn is_identity(zelf: &Py<Self>) -> bool {
            *zelf.values.read() == glam::Mat4::IDENTITY.to_cols_array()
        }
        #[pymethod(name = "setToIdentity")]
        fn set_identity(zelf: &Py<Self>, vm: &VirtualMachine) -> PyResult<()> {
            vm.host_budget_tick()?;
            *zelf.values.write() = glam::Mat4::IDENTITY.to_cols_array();
            Ok(())
        }
        #[pymethod]
        fn data(zelf: &Py<Self>, vm: &VirtualMachine) -> rustpython_vm::builtins::PyTupleRef {
            let values = *zelf.values.read();
            vm.ctx.new_tuple(
                values
                    .into_iter()
                    .map(|v| vm.ctx.new_float(v as f64).into())
                    .collect(),
            )
        }
        #[pymethod]
        fn translate(
            zelf: &Py<Self>,
            args: rustpython_vm::function::PosArgs<PyObjectRef>,
            vm: &VirtualMachine,
        ) -> PyResult<()> {
            vm.host_budget_tick()?;
            let [x, y, z] = Self::xyz(args, false, vm)?;
            let mut m = zelf.values.write();
            for r in 0..4 {
                m[12 + r] = m[r] * x + m[4 + r] * y + m[8 + r] * z + m[12 + r];
            }
            Ok(())
        }
        #[pymethod]
        fn scale(
            zelf: &Py<Self>,
            args: rustpython_vm::function::PosArgs<PyObjectRef>,
            vm: &VirtualMachine,
        ) -> PyResult<()> {
            vm.host_budget_tick()?;
            let scale = Self::xyz(args, true, vm)?;
            let mut m = zelf.values.write();
            for c in 0..3 {
                for r in 0..4 {
                    m[c * 4 + r] *= scale[c];
                }
            }
            Ok(())
        }
        #[pymethod]
        fn rotate(
            zelf: &Py<Self>,
            args: rustpython_vm::function::PosArgs<PyObjectRef>,
            vm: &VirtualMachine,
        ) -> PyResult<()> {
            vm.host_budget_tick()?;
            let args = args.into_vec();
            let (angle, axis) = match args.as_slice() {
                [a, v] => (
                    a.try_float(vm)?.to_f64() as f32,
                    v.downcast_ref::<crate::vm_groups::native_groups::QVector3D>()
                        .ok_or_else(|| vm.new_type_error("Rotation axis requires QVector3D"))?
                        .coordinates(),
                ),
                [a, x, y, z] => (
                    a.try_float(vm)?.to_f64() as f32,
                    [
                        x.try_float(vm)?.to_f64() as f32,
                        y.try_float(vm)?.to_f64() as f32,
                        z.try_float(vm)?.to_f64() as f32,
                    ],
                ),
                _ => {
                    return Err(vm.new_type_error("rotate requires angle and vector or angle,x,y,z"))
                }
            };
            if angle == 0. {
                return Ok(());
            }
            let (mut sin, cos) = match angle {
                90. | -270. => (1., 0.),
                -90. | 270. => (-1., 0.),
                180. | -180. => (0., -1.),
                _ => {
                    let radians = angle * (std::f64::consts::PI / 180.) as f32;
                    (radians.sin(), radians.cos())
                }
            };
            let [x, y, z] = axis;
            let mut m = zelf.values.write();
            let pair = if x == 0. && y == 0. && z != 0. {
                if z < 0. {
                    sin = -sin;
                }
                Some((0, 1))
            } else if x == 0. && y != 0. && z == 0. {
                if y < 0. {
                    sin = -sin;
                }
                Some((2, 0))
            } else if x != 0. && y == 0. && z == 0. {
                if x < 0. {
                    sin = -sin;
                }
                Some((1, 2))
            } else {
                None
            };
            if let Some((a, b)) = pair {
                for r in 0..4 {
                    let old = m[a * 4 + r];
                    m[a * 4 + r] = old * cos + m[b * 4 + r] * sin;
                    m[b * 4 + r] = m[b * 4 + r] * cos - old * sin;
                }
            } else {
                let squared = x as f64 * x as f64 + y as f64 * y as f64 + z as f64 * z as f64;
                let length = squared.sqrt();
                let (x, y, z) = if (squared - 1.).abs() * 1e12 > squared.abs().min(1.)
                    && squared.abs() > 1e-12
                {
                    (
                        (x as f64 / length) as f32,
                        (y as f64 / length) as f32,
                        (z as f64 / length) as f32,
                    )
                } else {
                    (x, y, z)
                };
                let ic = 1. - cos;
                let rot = [
                    x * x * ic + cos,
                    y * x * ic + z * sin,
                    x * z * ic - y * sin,
                    0.,
                    x * y * ic - z * sin,
                    y * y * ic + cos,
                    y * z * ic + x * sin,
                    0.,
                    x * z * ic + y * sin,
                    y * z * ic - x * sin,
                    z * z * ic + cos,
                    0.,
                    0.,
                    0.,
                    0.,
                    1.,
                ];
                let old = *m;
                for c in 0..4 {
                    for r in 0..4 {
                        m[c * 4 + r] = old[r] * rot[c * 4]
                            + old[4 + r] * rot[c * 4 + 1]
                            + old[8 + r] * rot[c * 4 + 2]
                            + old[12 + r] * rot[c * 4 + 3];
                    }
                }
            }
            Ok(())
        }
        #[pymethod]
        fn map(
            zelf: &Py<Self>,
            point: PyRef<crate::vm_groups::native_groups::QVector3D>,
            vm: &VirtualMachine,
        ) -> PyResult<crate::vm_groups::native_groups::QVector3D> {
            vm.host_budget_tick()?;
            let [x, y, z] = point.coordinates();
            let m = *zelf.values.read();
            let w = m[3] * x + m[7] * y + m[11] * z + m[15];
            let mut p = [0.; 3];
            for r in 0..3 {
                p[r] = (m[r] * x + m[4 + r] * y + m[8 + r] * z + m[12 + r]) / w;
            }
            Ok(crate::vm_groups::native_groups::QVector3D::from_coordinates(p))
        }
    }
    #[derive(FromArgs)]
    pub struct GroupArgs {
        #[pyarg(any, optional)]
        mesh: Option<PyObjectRef>,
        #[pyarg(any, optional)]
        name: Option<PyObjectRef>,
    }
    #[pyattr]
    #[pyclass(name = "Group", traverse)]
    #[derive(PyPayload)]
    pub struct Group {
        pub(super) mesh: PyRwLock<PyRef<editable_mesh::Mesh>>,
        pub(super) fields: PyDictRef,
        #[pytraverse(skip)]
        pub(super) id: uuid::Uuid,
        #[pytraverse(skip)]
        pub(super) baseline: ingetrazo_core::groups::Group,
        #[pytraverse(skip)]
        pub(super) _charge: Lease,
        #[pytraverse(skip)]
        pub(super) imported: bool,
    }
    impl std::fmt::Debug for Group {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("SceneGroup")
        }
    }
    impl Constructor for Group {
        type Args = GroupArgs;
        fn py_new(_cls: &Py<PyType>, args: GroupArgs, vm: &VirtualMachine) -> PyResult<Self> {
            vm.host_budget_tick()?;
            let budget = super::super::STAGED
                .with(|s| {
                    s.try_borrow()
                        .ok()
                        .and_then(|s| s.as_ref().map(|s| s.budget.clone()))
                })
                .ok_or_else(|| vm.new_value_error("Group requires an active owned transaction"))?;
            let charge = budget
                .reserve(Kind::Staging, 1024)
                .map_err(|e| vm.new_value_error(e))?;
            let mesh = if let Some(value) = args.mesh.filter(|v| !vm.is_none(v)) {
                value
                    .downcast::<editable_mesh::Mesh>()
                    .map_err(|_| vm.new_type_error("Group mesh requires Mesh"))?
            } else {
                let class = editable_mesh::Mesh::class(&vm.ctx).to_owned();
                class
                    .as_object()
                    .call((), vm)?
                    .downcast::<editable_mesh::Mesh>()
                    .map_err(|_| vm.new_type_error("Group Mesh constructor"))?
            };
            let name = match args.name {
                Some(v) if v.clone().is_true(vm)? => v,
                _ => vm
                    .ctx
                    .new_str(format!(
                        "Group {}",
                        GROUP_NUMBER.fetch_add(1, Ordering::Relaxed)
                    ))
                    .into(),
            };
            let id = uuid::Uuid::new_v4();
            let fields = vm.ctx.new_dict();
            fields.set_item("name", name, vm)?;
            fields.set_item(
                "uid",
                vm.ctx
                    .new_str(id.simple().to_string()[..16].to_owned())
                    .into(),
                vm,
            )?;
            fields.set_item("hidden", vm.ctx.new_bool(false).into(), vm)?;
            fields.set_item("component", vm.ctx.new_bool(true).into(), vm)?;
            fields.set_item("children", vm.ctx.new_list(vec![]).into(), vm)?;
            for key in [
                "layer",
                "material",
                "ifc",
                "xform",
                "axes",
                "text3d",
                "ext",
                "owner",
                "context",
                "exploded",
                "explode_offset",
            ] {
                fields.set_item(key, vm.ctx.none(), vm)?;
            }
            fields.set_item("billboard", vm.ctx.new_bool(false).into(), vm)?;
            Ok(Self {
                mesh: rustpython_vm::common::lock::PyRwLock::new(mesh),
                fields,
                id,
                baseline: ingetrazo_core::groups::Group::new(""),
                _charge: charge,
                imported: false,
            })
        }
    }
    impl Group {
        fn get(&self, key: &str, vm: &VirtualMachine) -> PyObjectRef {
            self.fields
                .get_item(key, vm)
                .unwrap_or_else(|_| vm.ctx.none())
        }
        fn set(&self, key: &str, value: PyObjectRef, vm: &VirtualMachine) -> PyResult<()> {
            vm.host_budget_tick()?;
            self.fields.set_item(key, value, vm)
        }
    }
    #[pyclass(with(Constructor))]
    impl Group {
        #[pygetset]
        fn mesh(zelf: &Py<Self>) -> PyRef<editable_mesh::Mesh> {
            zelf.mesh.read().clone()
        }
        #[pygetset(setter)]
        fn set_mesh(
            zelf: &Py<Self>,
            value: PyRef<editable_mesh::Mesh>,
            vm: &VirtualMachine,
        ) -> PyResult<()> {
            vm.host_budget_tick()?;
            let old = { std::mem::replace(&mut *zelf.mesh.write(), value) };
            drop(old);
            Ok(())
        }
        #[pygetset]
        fn name(zelf: &Py<Self>, vm: &VirtualMachine) -> PyObjectRef {
            zelf.get("name", vm)
        }
        #[pygetset(setter)]
        fn set_name(zelf: &Py<Self>, value: PyObjectRef, vm: &VirtualMachine) -> PyResult<()> {
            zelf.set("name", value, vm)
        }
        #[pygetset]
        fn layer(zelf: &Py<Self>, vm: &VirtualMachine) -> PyObjectRef {
            zelf.get("layer", vm)
        }
        #[pygetset(setter)]
        fn set_layer(zelf: &Py<Self>, value: PyObjectRef, vm: &VirtualMachine) -> PyResult<()> {
            zelf.set("layer", value, vm)
        }
        #[pygetset]
        fn hidden(zelf: &Py<Self>, vm: &VirtualMachine) -> PyObjectRef {
            zelf.get("hidden", vm)
        }
        #[pygetset(setter)]
        fn set_hidden(zelf: &Py<Self>, value: PyObjectRef, vm: &VirtualMachine) -> PyResult<()> {
            zelf.set("hidden", value, vm)
        }
        #[pygetset]
        fn material(zelf: &Py<Self>, vm: &VirtualMachine) -> PyObjectRef {
            zelf.get("material", vm)
        }
        #[pygetset(setter)]
        fn set_material(zelf: &Py<Self>, value: PyObjectRef, vm: &VirtualMachine) -> PyResult<()> {
            zelf.set("material", value, vm)
        }
        #[pygetset]
        fn uid(zelf: &Py<Self>, vm: &VirtualMachine) -> PyObjectRef {
            zelf.get("uid", vm)
        }
        #[pygetset]
        fn children(zelf: &Py<Self>, vm: &VirtualMachine) -> PyObjectRef {
            zelf.get("children", vm)
        }
        #[pygetset(setter)]
        fn set_children(zelf: &Py<Self>, value: PyObjectRef, vm: &VirtualMachine) -> PyResult<()> {
            zelf.set("children", value, vm)
        }
        #[pymethod]
        fn adopt(zelf: &Py<Self>, children: PyObjectRef, vm: &VirtualMachine) -> PyResult<()> {
            vm.host_budget_tick()?;
            let children = vm.extract_elements_with(&children, Ok)?;
            let nonempty = !children.is_empty();
            zelf.set("children", vm.ctx.new_list(children).into(), vm)?;
            if nonempty && vm.is_none(&zelf.get("xform", vm)) {
                zelf.set(
                    "xform",
                    QMatrix4x4::from_native(glam::DMat4::IDENTITY)
                        .expect("Finite exact identity")
                        .into_ref(&vm.ctx)
                        .into(),
                    vm,
                )?;
            }
            Ok(())
        }
        #[pygetset]
        fn xform(zelf: &Py<Self>, vm: &VirtualMachine) -> PyObjectRef {
            zelf.get("xform", vm)
        }
        #[pygetset(setter)]
        fn set_xform(zelf: &Py<Self>, value: PyObjectRef, vm: &VirtualMachine) -> PyResult<()> {
            zelf.set("xform", value, vm)
        }
        #[pygetset]
        fn component(zelf: &Py<Self>, vm: &VirtualMachine) -> PyObjectRef {
            zelf.get("component", vm)
        }
        #[pygetset(setter)]
        fn set_component(zelf: &Py<Self>, value: PyObjectRef, vm: &VirtualMachine) -> PyResult<()> {
            zelf.set("component", value, vm)
        }
        #[pymethod]
        fn is_component(zelf: &Py<Self>, vm: &VirtualMachine) -> PyResult<bool> {
            Ok(!vm.is_none(&zelf.get("xform", vm)) && zelf.get("component", vm).is_true(vm)?)
        }
    }
    /// Native orientation preparation for the actual editable graph. The recipe
    /// softening pass runs in the VM; no alternate helper Mesh/Group graph.
    #[pyfunction]
    fn orient_owned(mesh: PyRef<editable_mesh::Mesh>, vm: &VirtualMachine) -> PyResult<()> {
        vm.host_budget_tick()?;
        let budget = super::super::STAGED
            .with(|s| {
                s.try_borrow()
                    .ok()
                    .and_then(|s| s.as_ref().map(|s| s.budget.clone()))
            })
            .ok_or_else(|| {
                vm.new_value_error("Recipe orientation requires an owned transaction")
            })?;
        let result = (|| -> Result<(), String> {
            super::super::seed_packed_uids(&mesh)?;
            let mut live = RootRestore::prepare(mesh.clone(), &budget, vm)?;
            live.armed = false;
            let mut candidate = packed::prepare(
                &mesh,
                &live.vertices,
                &live.graph,
                &live.positions,
                &live.faces,
                &live.face_fields,
                &budget,
                vm,
            )?;
            // Explicit bounded native-cost prerequisite. This is not a wall-time
            // limit or the complete arbitrary orientation/partition API.
            if candidate.mesh.faces.len() > 2048 {
                return Err("Recipe native orientation face admission exceeded".into());
            }
            let mut count = 0usize;
            for face in &candidate.mesh.faces {
                if face.attrs.get("back").is_some_and(|v| v.is_object()) {
                    return Err("Two-sided recipe orientation publication remains required".into());
                }
                count = count
                    .checked_add(face.vertices.len())
                    .ok_or("Recipe ring extent overflow")?;
            }
            let _charge = budget.reserve(
                Kind::Preparation,
                count
                    .checked_mul(std::mem::size_of::<usize>())
                    .and_then(|n| {
                        n.checked_add(
                            candidate.mesh.faces.len() * std::mem::size_of::<Vec<usize>>(),
                        )
                    })
                    .ok_or("Recipe preparation extent overflow")?,
            )?;
            let mut old = Vec::new();
            old.try_reserve_exact(candidate.mesh.faces.len())
                .map_err(|_| "Recipe orientation rows refused")?;
            for face in &candidate.mesh.faces {
                let mut row = Vec::new();
                row.try_reserve_exact(face.vertices.len())
                    .map_err(|_| "Recipe winding preparation refused")?;
                row.extend_from_slice(&face.vertices);
                old.push(row);
            }
            if !candidate.mesh.faces.is_empty()
                && !crate::vm_groups::native_groups::source_coplanar(&candidate.mesh)
            {
                ingetrazo_core::heal::orient_boundary_candidate(&mut candidate.mesh)?;
            }
            if candidate.mesh.faces.iter().any(|f| f.interior) {
                return Err("Recipe partition field projection remains required".into());
            }
            // All fallible preparation precedes in-place ring reversal. Existing
            // persistent list objects and held Face/Vertex identities survive.
            for ((face, before), after) in
                live.face_fields.iter().zip(&old).zip(&candidate.mesh.faces)
            {
                if before != &after.vertices {
                    if !before
                        .iter()
                        .rev()
                        .copied()
                        .eq(after.vertices.iter().copied())
                    {
                        return Err("Recipe kernel changed unsupported topology".into());
                    }
                }
                for ring in face.face.hole_loops.read().borrow_vec().iter() {
                    if ring
                        .downcast_ref::<rustpython_vm::builtins::PyList>()
                        .is_none()
                    {
                        return Err("Recipe hole container requires list".into());
                    }
                }
            }
            // Compare packed face winding with the saved genuine graph rows.
            for ((face, native), before) in
                live.face_fields.iter().zip(&candidate.mesh.faces).zip(&old)
            {
                if before != &native.vertices {
                    let ring = face.face.r#loop.read().clone();
                    ring.borrow_vec_mut().reverse();
                    let holes = face.face.hole_loops.read().clone();
                    for hole in holes.borrow_vec().iter() {
                        hole.downcast_ref::<rustpython_vm::builtins::PyList>()
                            .unwrap()
                            .borrow_vec_mut()
                            .reverse();
                    }
                }
            }
            Ok(())
        })();
        if result.is_err() {
            super::super::STAGED.with(|s| {
                if let Ok(mut s) = s.try_borrow_mut() {
                    if let Some(stage) = s.as_mut() {
                        stage.failed = true;
                    }
                }
            });
        }
        result.map_err(|e| vm.new_value_error(e))
    }
    #[pyattr]
    #[pyclass(name = "Scene", traverse)]
    #[derive(Debug, PyPayload)]
    pub struct Scene {
        pub(in super::super) mesh: PyRef<editable_mesh::Mesh>,
        pub(in super::super) groups: PyListRef,
    }
    #[pyclass]
    impl Scene {
        #[pygetset]
        fn mesh(zelf: &Py<Self>) -> PyRef<editable_mesh::Mesh> {
            zelf.mesh.clone()
        }
        #[pygetset]
        fn groups(zelf: &Py<Self>) -> PyListRef {
            zelf.groups.clone()
        }
    }
}

/// Source SnapshotImport owns only added memberships, retaining exact objects.
pub(super) struct Membership {
    pub scene: PyRef<console_scene::Scene>,
    pub added: Vec<PyObjectRef>,
    _charge: Lease,
}
impl Membership {
    pub fn prepare(
        scene: PyRef<console_scene::Scene>,
        before: &[PyObjectRef],
        budget: &OwnerBudget,
    ) -> Result<Self, String> {
        let count = scene.groups.borrow_vec().len();
        let charge = budget.reserve(
            Kind::History,
            count
                .checked_mul(std::mem::size_of::<PyObjectRef>())
                .ok_or("Group membership extent overflow")?,
        )?;
        let mut added = Vec::new();
        added
            .try_reserve_exact(count)
            .map_err(|_| "Group command members refused")?;
        {
            let values = scene.groups.borrow_vec();
            for value in values.iter() {
                if value.downcast_ref::<console_scene::Group>().is_none() {
                    return Err("Scene groups require genuine Group".into());
                }
                if !before.iter().any(|old| old.is(value)) {
                    added.push(value.clone());
                }
            }
        }
        Ok(Self {
            scene,
            added,
            _charge: charge,
        })
    }
    pub fn prepare_history(&self, redo: bool) -> Result<(), String> {
        if redo {
            self.scene
                .groups
                .borrow_vec_mut()
                .try_reserve(self.added.len())
                .map_err(|_| "Group Redo list capacity refused")?;
        }
        Ok(())
    }
    pub fn apply(&self, redo: bool) {
        if redo {
            let mut groups = self.scene.groups.borrow_vec_mut();
            for g in &self.added {
                if !groups.iter().any(|v| v.is(g)) {
                    groups.push(g.clone());
                }
            }
        } else {
            for g in &self.added {
                let removed = {
                    let mut groups = self.scene.groups.borrow_vec_mut();
                    groups
                        .iter()
                        .position(|v| v.is(g))
                        .map(|i| groups.remove(i))
                };
                drop(removed);
            }
        }
    }
}

pub(super) fn members(
    scene: &PyRef<console_scene::Scene>,
    budget: &OwnerBudget,
) -> Result<(Vec<PyObjectRef>, Lease), String> {
    let count = scene.groups.borrow_vec().len();
    let charge = budget.reserve(
        Kind::Preparation,
        count
            .checked_mul(std::mem::size_of::<PyObjectRef>())
            .ok_or("Group snapshot extent overflow")?,
    )?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| "Group snapshot references refused")?;
    values.extend(scene.groups.borrow_vec().iter().cloned());
    Ok((values, charge))
}

/// Prepared full group-list replacement. Mesh packing and metadata work remain
/// O(group geometry), not O(changed); all candidates precede application writes.
pub struct GroupPublication {
    pub groups: Vec<ingetrazo_core::groups::Group>,
    pub shares: std::collections::HashMap<uuid::Uuid, ingetrazo_core::mesh::Mesh>,
    pub packed_meshes: usize,
    pub definitions:
        std::collections::HashMap<uuid::Uuid, ingetrazo_core::components::ComponentDefinition>,
    pub charge: Lease,
}
fn str_value(
    value: PyObjectRef,
    vm: &VirtualMachine,
    label: &str,
) -> Result<Option<String>, String> {
    if vm.is_none(&value) {
        return Ok(None);
    }
    let s = value
        .downcast_ref::<rustpython_vm::builtins::PyStr>()
        .ok_or_else(|| format!("Native {label} requires text"))?;
    Ok(Some(
        s.as_utf8()
            .ok_or("Native group Unicode string pending")?
            .as_str()
            .to_owned(),
    ))
}
pub(super) fn publication(
    scene: &PyRef<console_scene::Scene>,
    budget: &OwnerBudget,
    vm: &VirtualMachine,
) -> Result<GroupPublication, String> {
    let (values, mut charge) = members(scene, budget)?;
    charge.resize(
        values
            .len()
            .checked_mul(2048)
            .ok_or("Group receipt extent overflow")?,
    )?;
    let mut shares = std::collections::HashMap::<uuid::Uuid, ingetrazo_core::mesh::Mesh>::new();
    let mut definitions = std::collections::HashMap::new();
    definitions
        .try_reserve(values.len())
        .map_err(|_| "Group definition references refused")?;
    let mut costs = std::collections::HashMap::<uuid::Uuid, usize>::new();
    costs
        .try_reserve(values.len())
        .map_err(|_| "Group receipt cost references refused")?;
    shares
        .try_reserve(values.len())
        .map_err(|_| "Group shares capacity refused")?;
    let groups = publish_groups(
        values,
        budget,
        vm,
        &mut charge,
        &mut shares,
        &mut costs,
        &mut definitions,
        &mut Vec::new(),
    )?;
    Ok(GroupPublication {
        groups,
        packed_meshes: costs.len(),
        definitions,
        shares,
        charge,
    })
}

fn publish_groups(
    values: Vec<PyObjectRef>,
    budget: &OwnerBudget,
    vm: &VirtualMachine,
    charge: &mut Lease,
    shares: &mut std::collections::HashMap<uuid::Uuid, ingetrazo_core::mesh::Mesh>,
    costs: &mut std::collections::HashMap<uuid::Uuid, usize>,
    definitions: &mut std::collections::HashMap<
        uuid::Uuid,
        ingetrazo_core::components::ComponentDefinition,
    >,
    path: &mut Vec<usize>,
) -> Result<Vec<ingetrazo_core::groups::Group>, String> {
    if path.len() >= 128 {
        return Err("Group nesting admission exceeded".into());
    }
    path.try_reserve(1)
        .map_err(|_| "Group ancestry preparation refused")?;
    let mut groups = Vec::new();
    groups
        .try_reserve_exact(values.len())
        .map_err(|_| "Group publication capacity refused")?;
    for value in values {
        let identity = value.get_id();
        if path.contains(&identity) {
            return Err("Cyclic Group children cannot be published".into());
        }
        path.push(identity);
        charge.merge(budget.reserve(Kind::Preparation, 2048)?)?;
        shares
            .try_reserve(1)
            .map_err(|_| "Group share references refused")?;
        costs
            .try_reserve(1)
            .map_err(|_| "Group cost references refused")?;
        definitions
            .try_reserve(1)
            .map_err(|_| "Group definitions refused")?;
        let group = value
            .downcast::<console_scene::Group>()
            .map_err(|_| "Scene groups require genuine Group")?;
        let placement = group
            .fields
            .get_item("xform", vm)
            .map_err(|_| "Group placement missing")?;
        if !vm.is_none(&placement)
            && placement
                .downcast_ref::<console_scene::QMatrix4x4>()
                .is_none()
        {
            return Err("Nonidentity Group transforms are not yet admitted".into());
        }
        let children = group
            .fields
            .get_item("children", vm)
            .map_err(|_| "Group children missing")?
            .downcast::<rustpython_vm::builtins::PyList>()
            .map_err(|_| "Group children require list")?;
        let count = children.borrow_vec().len();
        let _refs = budget.reserve(
            Kind::Preparation,
            count
                .checked_mul(std::mem::size_of::<PyObjectRef>())
                .ok_or("Child extent overflow")?,
        )?;
        let mut child_values = Vec::new();
        child_values
            .try_reserve_exact(count)
            .map_err(|_| "Child references refused")?;
        child_values.extend(children.borrow_vec().iter().cloned());
        let native_children = publish_groups(
            child_values,
            budget,
            vm,
            charge,
            shares,
            costs,
            definitions,
            path,
        )?;
        let mesh = group.mesh.read().clone();
        let token = mesh.share_token.get().unwrap_or_else(|| {
            let token = uuid::Uuid::new_v4();
            mesh.share_token.set(Some(token));
            token
        });
        let (mesh_receipt, share_bytes) = if let Some(existing) = shares.get(&token) {
            let bytes = costs[&token];
            charge.merge(budget.reserve(Kind::Preparation, bytes)?)?;
            (existing.clone(), bytes)
        } else {
            super::seed_packed_uids(&mesh)?;
            let mut live = RootRestore::prepare(mesh.clone(), budget, vm)?;
            live.armed = false;
            let candidate = packed::prepare(
                &mesh,
                &live.vertices,
                &live.graph,
                &live.positions,
                &live.faces,
                &live.face_fields,
                budget,
                vm,
            )?;
            let (receipt, receipt_charge) = candidate.into_mesh_receipt()?;
            let bytes = receipt_charge.bytes();
            charge.merge(receipt_charge)?;
            costs.insert(token, bytes);
            (receipt, bytes)
        };
        let name = str_value(
            group
                .fields
                .get_item("name", vm)
                .map_err(|_| "Group name missing")?,
            vm,
            "group name",
        )?
        .ok_or("Native group name requires text")?;
        let mut native = ingetrazo_core::groups::Group::with_mesh(name, mesh_receipt);
        native.id = group.id;
        native.children = native_children;
        native.layer = str_value(
            group
                .fields
                .get_item("layer", vm)
                .map_err(|_| "Group layer missing")?,
            vm,
            "group layer",
        )?;
        let hidden = group
            .fields
            .get_item("hidden", vm)
            .map_err(|_| "Group visibility missing")?;
        native.hidden = if hidden.is(&vm.ctx.true_value) {
            true
        } else if hidden.is(&vm.ctx.false_value) {
            false
        } else {
            return Err("Native Group visibility currently requires bool".into());
        };
        native.transform = if vm.is_none(&placement) {
            glam::DMat4::IDENTITY
        } else {
            placement
                .downcast_ref::<console_scene::QMatrix4x4>()
                .unwrap()
                .native()?
        };
        native.mesh_share = Some(token);
        let component = group
            .fields
            .get_item("component", vm)
            .map_err(|_| "Group component flag missing")?;
        if !component.is(&vm.ctx.true_value) && !component.is(&vm.ctx.false_value) {
            return Err("Native component flags currently require bool".into());
        }
        native.component_definition = if !vm.is_none(&placement) && component.is(&vm.ctx.true_value)
        {
            let definition_id = if group.baseline.mesh_share == Some(token) {
                group.baseline.component_definition.unwrap_or(token)
            } else {
                token
            };
            if !definitions.contains_key(&definition_id) {
                let mut definition_bytes = share_bytes;
                fn child_bytes(
                    children: &[ingetrazo_core::groups::Group],
                    costs: &std::collections::HashMap<uuid::Uuid, usize>,
                ) -> Result<usize, String> {
                    let mut total = 0usize;
                    for child in children {
                        let mesh = child
                            .mesh_share
                            .and_then(|token| costs.get(&token))
                            .copied()
                            .ok_or("Child mesh receipt cost missing")?;
                        total = total
                            .checked_add(mesh)
                            .and_then(|n| n.checked_add(2048))
                            .and_then(|n| n.checked_add(child_bytes(&child.children, costs).ok()?))
                            .ok_or("Definition child receipt overflow")?;
                    }
                    Ok(total)
                }
                definition_bytes = definition_bytes
                    .checked_add(child_bytes(&native.children, costs)?)
                    .ok_or("Definition receipt extent overflow")?;
                charge.merge(budget.reserve(Kind::Preparation, definition_bytes)?)?;
                definitions.insert(
                    definition_id,
                    ingetrazo_core::components::ComponentDefinition {
                        mesh_share: Some(token),
                        mesh: native.mesh.clone(),
                        children: native.children.clone(),
                    },
                );
            }
            Some(definition_id)
        } else {
            None
        };
        if let Some(token) = native.mesh_share {
            if let Some(existing) = shares.get(&token) {
                if existing != &native.mesh {
                    return Err(
                        "Shared Group mesh identity requires coherent native geometry".into(),
                    );
                }
            } else {
                // Registry owns an independent packed native mesh until the core
                // scene gains shared immutable storage. Admit before cloning.
                charge
                    .merge(budget.reserve(crate::owner_budget::Kind::Preparation, share_bytes)?)?;
                shares.insert(token, native.mesh.clone());
            }
        }
        native.ifc_tag = group.baseline.ifc_tag.clone();
        native.billboard = group.baseline.billboard;
        native.billboard_texture = group.baseline.billboard_texture.clone();
        native.definition_member = group.baseline.definition_member;
        native.native_document = group.baseline.native_document.clone();
        native.native_baseline = group.baseline.native_baseline.clone();
        native.history_id = group.baseline.history_id;
        let material = group
            .fields
            .get_item("material", vm)
            .map_err(|_| "Group paint missing")?;
        if !vm.is_none(&material) {
            let fields = super::face_fields::prepare(&material, budget, vm)?;
            charge.merge(fields.charge)?;
            native.native_document["material"] = serde_json::to_value(fields.attrs)
                .map_err(|_| "Native group paint encoding refused")?;
        } else if let Some(fields) = native.native_document.as_object_mut() {
            fields.remove("material");
        }
        if native.native_document.is_null() {
            native.native_document = serde_json::json!({});
        }
        native.native_document["uid"] = serde_json::Value::String(
            str_value(
                group
                    .fields
                    .get_item("uid", vm)
                    .map_err(|_| "Group uid missing")?,
                vm,
                "group uid",
            )?
            .ok_or("Group uid requires text")?,
        );
        groups.push(native);
        path.pop();
    }
    Ok(groups)
}

pub(super) fn import(
    scene: &ingetrazo_core::scene::Scene,
    root: PyRef<editable_mesh::Mesh>,
    budget: &OwnerBudget,
    vm: &VirtualMachine,
) -> Result<PyRef<console_scene::Scene>, String> {
    let mut owners = std::collections::HashMap::<uuid::Uuid, PyRef<editable_mesh::Mesh>>::new();
    owners
        .try_reserve(scene.groups.len())
        .map_err(|_| "Imported Mesh owner references refused")?;
    let groups = import_groups(scene, &scene.groups, budget, vm, &mut owners, 0)?;
    Ok(console_scene::Scene {
        mesh: root,
        groups: vm.ctx.new_list(groups),
    }
    .into_ref(&vm.ctx))
}
fn import_groups(
    scene: &ingetrazo_core::scene::Scene,
    native_groups: &[ingetrazo_core::groups::Group],
    budget: &OwnerBudget,
    vm: &VirtualMachine,
    owners: &mut std::collections::HashMap<uuid::Uuid, PyRef<editable_mesh::Mesh>>,
    depth: usize,
) -> Result<Vec<PyObjectRef>, String> {
    if depth >= 128 {
        return Err("Group nesting admission exceeded".into());
    }
    owners
        .try_reserve(native_groups.len())
        .map_err(|_| "Imported Mesh owner references refused")?;
    let mut groups = Vec::new();
    groups
        .try_reserve_exact(native_groups.len())
        .map_err(|_| "Imported child references refused")?;
    for g in native_groups {
        if let Some(id) = g.component_definition {
            let definition = scene
                .component_definitions
                .get(&id)
                .ok_or("Imported component definition is missing")?;
            // The native IGZ loader publishes prototype membership on Group;
            // its definition snapshot intentionally has no mesh_share token.
            // Only an explicitly linked definition must agree with that owner.
            if definition
                .mesh_share
                .is_some_and(|token| Some(token) != g.mesh_share)
                || definition.mesh != g.mesh
                || g.native_document
                    .get("component")
                    .is_some_and(|v| v != &serde_json::Value::Bool(true))
            {
                return Err("Imported component definition is not coherent".into());
            }
        }
        let mesh = if let Some(token) = g.mesh_share {
            if scene.mesh_shares.get(&token) != Some(&g.mesh) {
                return Err("Imported shared Mesh registry is not coherent".into());
            }
            if let Some(mesh) = owners.get(&token) {
                mesh.clone()
            } else {
                let mesh = hydrate::prepare(g.mesh.clone(), budget, vm)?;
                mesh.share_token.set(Some(token));
                owners.insert(token, mesh.clone());
                mesh
            }
        } else {
            hydrate::prepare(g.mesh.clone(), budget, vm)?
        };
        mesh.managed.set(false);
        let fields = vm.ctx.new_dict();
        fields
            .set_item("name", vm.ctx.new_str(g.name.clone()).into(), vm)
            .map_err(|_| "Group name import refused")?;
        fields
            .set_item("uid", vm.ctx.new_str(g.native_uid()).into(), vm)
            .map_err(|_| "Group uid import refused")?;
        fields
            .set_item(
                "layer",
                g.layer
                    .as_ref()
                    .map(|v| vm.ctx.new_str(v.clone()).into())
                    .unwrap_or_else(|| vm.ctx.none()),
                vm,
            )
            .map_err(|_| "Group layer import refused")?;
        fields
            .set_item("hidden", vm.ctx.new_bool(g.hidden).into(), vm)
            .map_err(|_| "Group visibility import refused")?;
        fields
            .set_item(
                "children",
                vm.ctx
                    .new_list(import_groups(
                        scene,
                        &g.children,
                        budget,
                        vm,
                        owners,
                        depth + 1,
                    )?)
                    .into(),
                vm,
            )
            .map_err(|_| "Group children import refused")?;
        fields
            .set_item("component", vm.ctx.new_bool(true).into(), vm)
            .map_err(|_| "Group flags import refused")?;
        for key in [
            "material",
            "ifc",
            "xform",
            "axes",
            "text3d",
            "ext",
            "owner",
            "context",
            "exploded",
            "explode_offset",
        ] {
            fields
                .set_item(key, vm.ctx.none(), vm)
                .map_err(|_| "Group defaults import refused")?;
        }
        if let Some(material) = g.native_document.get("material") {
            fields
                .set_item("material", hydrate::value(material, vm)?, vm)
                .map_err(|_| "Group paint import refused")?;
        }
        if !g.children.is_empty()
            || g.transform != glam::DMat4::IDENTITY
            || g.component_definition.is_some()
            || g.native_document.get("xform").is_some_and(|v| !v.is_null())
        {
            fields
                .set_item(
                    "xform",
                    console_scene::QMatrix4x4::from_native(g.transform)?
                        .into_ref(&vm.ctx)
                        .into(),
                    vm,
                )
                .map_err(|_| "Group identity placement import refused")?;
        }
        let component = g
            .native_document
            .get("component")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        fields
            .set_item("component", vm.ctx.new_bool(component).into(), vm)
            .map_err(|_| "Group component flag import refused")?;
        let charge = budget.reserve(Kind::History, 2048)?;
        groups.push(
            console_scene::Group {
                mesh: rustpython_vm::common::lock::PyRwLock::new(mesh),
                fields,
                id: g.id,
                // The VM owns genuine Mesh/child references separately. Preserve
                // placement metadata, never clone the full nested native graph
                // into every ancestor's baseline.
                baseline: ingetrazo_core::groups::Group {
                    history_id: g.history_id,
                    mesh_share: g.mesh_share,
                    component_definition: g.component_definition,
                    definition_member: g.definition_member,
                    native_document: g.native_document.clone(),
                    native_baseline: g.native_baseline.clone(),
                    id: g.id,
                    name: g.name.clone(),
                    mesh: ingetrazo_core::mesh::Mesh::new(),
                    transform: g.transform,
                    layer: g.layer.clone(),
                    children: vec![],
                    ifc_tag: g.ifc_tag.clone(),
                    billboard: g.billboard,
                    billboard_texture: g.billboard_texture.clone(),
                    hidden: g.hidden,
                },
                _charge: charge,
                imported: true,
            }
            .into_ref(&vm.ctx)
            .into(),
        );
    }
    Ok(groups)
}
