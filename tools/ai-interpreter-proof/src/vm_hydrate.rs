//! Cold import of actual native mesh records into genuine VM object owners.
//! Still standalone: this is not a complete Scene bridge or total VM memory cap.
use super::{
    editable_mesh::{Edge, Face, Mesh, Vertex},
    source_registry_key,
};
use crate::{
    face_owner::FaceOwner,
    owner_budget::{Kind, OwnerBudget},
    vm_groups::native_groups::QVector3D,
};
use rustpython_vm::{
    AsObject, PyObjectRef, PyPayload, PyRef, VirtualMachine,
    builtins::{PyList, PySet},
    common::lock::PyRwLock,
};
pub(super) fn value(v: &serde_json::Value, vm: &VirtualMachine) -> Result<PyObjectRef, String> {
    Ok(match v {
        serde_json::Value::Null => vm.ctx.none(),
        serde_json::Value::Bool(v) => vm.ctx.new_bool(*v).into(),
        serde_json::Value::String(v) => vm.ctx.new_str(v.clone()).into(),
        serde_json::Value::Number(v) => {
            if let Some(v) = v.as_i64() {
                vm.ctx.new_int(v).into()
            } else if let Some(v) = v.as_u64() {
                vm.ctx.new_int(v).into()
            } else {
                vm.ctx
                    .new_float(v.as_f64().ok_or("Native float invalid")?)
                    .into()
            }
        }
        serde_json::Value::Array(v) => {
            let mut out = Vec::new();
            out.try_reserve_exact(v.len())
                .map_err(|_| "Native array import refused")?;
            for v in v {
                out.push(value(v, vm)?);
            }
            vm.ctx.new_list(out).into()
        }
        serde_json::Value::Object(v) => {
            let out = vm.ctx.new_dict();
            for (k, v) in v {
                out.set_item(k, value(v, vm)?, vm)
                    .map_err(|_| "Native dict import refused")?;
            }
            out.into()
        }
    })
}
pub(super) fn prepare(
    native: ingetrazo_core::mesh::Mesh,
    budget: &OwnerBudget,
    vm: &VirtualMachine,
) -> Result<PyRef<Mesh>, String> {
    if native.edges.iter().any(|e| !e.attrs.is_empty()) {
        return Err("Native opaque Edge import projection pending".into());
    }
    let count = native
        .vertices
        .len()
        .checked_add(native.edges.len())
        .and_then(|n| n.checked_add(native.faces.len()))
        .ok_or("Native import extent overflow")?;
    let mut bytes = count
        .checked_mul(512)
        .ok_or("Native import extent overflow")?;
    for f in &native.faces {
        bytes = bytes
            .checked_add(crate::face_owner::attribute_payload(&f.attrs)?)
            .and_then(|n| n.checked_add(f.layer.as_ref().map_or(0, String::len)))
            .ok_or("Native import attrs extent overflow")?;
    }
    let charge = budget.reserve(Kind::Preparation, bytes)?;
    let mut vertices: Vec<PyRef<Vertex>> = Vec::new();
    vertices
        .try_reserve_exact(native.vertices.len())
        .map_err(|_| "Native Vertex import refused")?;
    for v in &native.vertices {
        if !v.position.is_finite() {
            return Err("Native nonfinite Vertex refused".into());
        }
        let p = v.position.to_array().map(|x| x as f32);
        if p.iter().any(|x| !x.is_finite()) {
            return Err("Native Qt Vertex extent refused".into());
        }
        vertices.push(
            Vertex {
                packed_uid: std::cell::Cell::new(Some(v.uid)),
                position: PyRwLock::new(QVector3D::from_coordinates(p).into_ref(&vm.ctx)),
                edges: PyRwLock::new(Some(PySet::default().into_ref(&vm.ctx).into())),
            }
            .track()
            .into_ref(&vm.ctx),
        );
    }
    let ring = |ids: &[usize]| -> Result<_, String> {
        let mut out = Vec::new();
        out.try_reserve_exact(ids.len())
            .map_err(|_| "Native ring import refused")?;
        for id in ids {
            out.push(
                vertices
                    .get(*id)
                    .ok_or("Native ring Vertex invalid")?
                    .clone()
                    .into(),
            );
        }
        Ok(vm.ctx.new_list(out))
    };
    let mut faces = Vec::new();
    faces
        .try_reserve_exact(native.faces.len())
        .map_err(|_| "Native Face import refused")?;
    for f in &native.faces {
        if f.interior {
            return Err("Native interior Face import pending".into());
        }
        let mut holes = Vec::new();
        holes
            .try_reserve_exact(f.holes.len())
            .map_err(|_| "Native holes import refused")?;
        for h in &f.holes {
            holes.push(ring(h)?.into());
        }
        let attrs = vm.ctx.new_dict();
        for (k, v) in &f.attrs {
            attrs
                .set_item(k, value(v, vm)?, vm)
                .map_err(|_| "Native attrs import refused")?;
        }
        if let Some(layer) = &f.layer {
            attrs
                .set_item("layer", vm.ctx.new_str(layer.clone()).into(), vm)
                .map_err(|_| "Native layer import refused")?;
        }
        faces.push(
            Face {
                r#loop: PyRwLock::new(ring(&f.vertices)?),
                hole_loops: PyRwLock::new(vm.ctx.new_list(holes)),
                attrs: PyRwLock::new(attrs.into()),
            }
            .into_ref(&vm.ctx),
        );
    }
    let mut edges = Vec::new();
    edges
        .try_reserve_exact(native.edges.len())
        .map_err(|_| "Native Edge import refused")?;
    for e in &native.edges {
        let a = vertices.get(e.v0).ok_or("Native Edge endpoint invalid")?;
        let b = vertices.get(e.v1).ok_or("Native Edge endpoint invalid")?;
        let edge = Edge::generated(a.clone(), b.clone(), vm);
        let mut radial = Vec::new();
        radial
            .try_reserve_exact(e.faces.len())
            .map_err(|_| "Native radial import refused")?;
        for id in &e.faces {
            radial.push(
                faces
                    .get(*id)
                    .ok_or("Native radial Face invalid")?
                    .clone()
                    .into(),
            );
        }
        *edge.faces.write() = Some(vm.ctx.new_list(radial).into());
        *edge.soft.write() = Some(vm.ctx.new_bool(e.soft).into());
        *edge.hidden.write() = Some(vm.ctx.new_bool(e.hidden).into());
        *edge.curve.write() = Some(
            e.curve_id
                .map(|v| vm.ctx.new_int(v).into())
                .unwrap_or_else(|| vm.ctx.none()),
        );
        *edge.layer.write() = Some(
            e.layer
                .as_ref()
                .map(|v| vm.ctx.new_str(v.clone()).into())
                .unwrap_or_else(|| vm.ctx.none()),
        );
        for vertex in [a, b] {
            let set = vertex
                .edges
                .read()
                .clone()
                .unwrap()
                .downcast::<PySet>()
                .unwrap();
            set.add(edge.clone().into(), vm)
                .map_err(|_| "Native incidence import refused")?;
        }
        edges.push(edge);
    }
    let registry = vm.ctx.new_dict();
    for v in &vertices {
        let p = v.position.read().coordinates().map(f64::from);
        let key = source_registry_key(glam::DVec3::from_array(p), vm)
            .map_err(|_| "Native registry key refused")?;
        registry
            .set_item(key.as_object(), v.clone().into(), vm)
            .map_err(|_| "Native registry import refused")?;
    }
    let owner = FaceOwner::new_with_budget(native, budget.clone())?;
    let result = Mesh {
        owner: PyRwLock::new(owner),
        vertices: vm
            .ctx
            .new_list(vertices.into_iter().map(Into::into).collect()),
        edges: vm.ctx.new_list(edges.into_iter().map(Into::into).collect()),
        faces: vm.ctx.new_list(faces.into_iter().map(Into::into).collect()),
        registry,
        managed: std::cell::Cell::new(true),
        native_field_charge: PyRwLock::new(Some(charge)),
        opaque_native_fields: std::cell::Cell::new(0),
    }
    .into_ref(&vm.ctx);
    Ok(result)
}
