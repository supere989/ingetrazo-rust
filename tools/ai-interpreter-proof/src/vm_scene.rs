//! Persistent Scene/group aliases for the opt-in application console.
//! Classic group membership is command-owned; existing group contents are not.
use super::{RootRestore, editable_mesh, hydrate, packed};
use crate::owner_budget::{Kind, Lease, OwnerBudget};
use rustpython_vm::{AsObject, PyObjectRef, PyPayload, PyRef, VirtualMachine, builtins::PyListRef};
use std::sync::atomic::{AtomicU64, Ordering};
static GROUP_NUMBER: AtomicU64 = AtomicU64::new(1);

#[rustpython_vm::pymodule]
pub(super) mod console_scene {
    use super::*;
    use rustpython_vm::{
        FromArgs, Py, PyResult, Traverse,
        builtins::{PyDictRef, PyType},
        common::lock::PyRwLock,
        pyclass,
        types::Constructor,
    };
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
                mesh: PyRwLock::new(mesh),
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
        #[pygetset]
        fn xform(zelf: &Py<Self>, vm: &VirtualMachine) -> PyObjectRef {
            zelf.get("xform", vm)
        }
        #[pygetset]
        fn component(zelf: &Py<Self>, vm: &VirtualMachine) -> PyObjectRef {
            zelf.get("component", vm)
        }
        #[pymethod]
        fn is_component(zelf: &Py<Self>, vm: &VirtualMachine) -> PyResult<bool> {
            Ok(!vm.is_none(&zelf.get("xform", vm)) && zelf.get("component", vm).is_true(vm)?)
        }
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
    let mut groups = Vec::new();
    groups
        .try_reserve_exact(values.len())
        .map_err(|_| "Native group receipt capacity refused")?;
    for value in values {
        let group = value
            .downcast::<console_scene::Group>()
            .map_err(|_| "Scene groups require genuine Group")?;
        let placement = group
            .fields
            .get_item("xform", vm)
            .map_err(|_| "Group placement missing")?;
        if !vm.is_none(&placement) {
            return Err("Group transforms are not yet admitted".into());
        }
        let children = group
            .fields
            .get_item("children", vm)
            .map_err(|_| "Group children missing")?
            .downcast::<rustpython_vm::builtins::PyList>()
            .map_err(|_| "Group children require list")?;
        if !children.borrow_vec().is_empty() {
            return Err("Nested Group publication remains required".into());
        }
        let mesh = group.mesh.read().clone();
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
        let (mesh_receipt, receipt_charge) = candidate.into_mesh_receipt()?;
        charge.merge(receipt_charge)?;
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
        native.transform = group.baseline.transform;
        native.mesh_share = if group.imported {
            group.baseline.mesh_share
        } else {
            Some(uuid::Uuid::from_u128(
                scene.get_id() as u128 ^ ((mesh.get_id() as u128) << 64),
            ))
        };
        native.ifc_tag = group.baseline.ifc_tag.clone();
        native.billboard = group.baseline.billboard;
        native.billboard_texture = group.baseline.billboard_texture.clone();
        native.definition_member = group.baseline.definition_member;
        native.native_document = group.baseline.native_document.clone();
        native.native_baseline = group.baseline.native_baseline.clone();
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
    }
    Ok(GroupPublication { groups, charge })
}

pub(super) fn import(
    scene: &ingetrazo_core::scene::Scene,
    root: PyRef<editable_mesh::Mesh>,
    budget: &OwnerBudget,
    vm: &VirtualMachine,
) -> Result<PyRef<console_scene::Scene>, String> {
    use rustpython_vm::common::lock::PyRwLock;
    let mut groups = Vec::new();
    groups
        .try_reserve_exact(scene.groups.len())
        .map_err(|_| "Imported Group references refused")?;
    for g in &scene.groups {
        if !g.children.is_empty()
            || g.component_definition.is_some()
            || g.transform != glam::DMat4::IDENTITY
            || g.native_document.get("proto").is_some_and(|p| !p.is_null())
            || g.native_document.get("xform").is_some_and(|p| !p.is_null())
        {
            return Err("Placed/nested Group hydration remains required".into());
        }
        let mesh = hydrate::prepare(g.mesh.clone(), budget, vm)?;
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
            .set_item("children", vm.ctx.new_list(vec![]).into(), vm)
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
        let charge = budget.reserve(Kind::History, 2048)?;
        groups.push(
            console_scene::Group {
                mesh: PyRwLock::new(mesh),
                fields,
                id: g.id,
                baseline: g.clone(),
                _charge: charge,
                imported: true,
            }
            .into_ref(&vm.ctx)
            .into(),
        );
    }
    Ok(console_scene::Scene {
        mesh: root,
        groups: vm.ctx.new_list(groups),
    }
    .into_ref(&vm.ctx))
}
