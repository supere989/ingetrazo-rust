//! One-way native field projection; VM objects remain the authoritative owners.
//! No Python callback or JSON encode/decode round-trip is used for these fields.
use crate::owner_budget::{Kind, Lease, OwnerBudget};
use num_traits::ToPrimitive;
use rustpython_vm::{
    AsObject, PyObjectRef, VirtualMachine,
    builtins::{PyDict, PyFloat, PyInt, PyList, PyStr, PyTuple},
};
use serde_json::Value;
use std::collections::HashMap;
pub(super) struct Fields {
    pub attrs: HashMap<String, Value>,
    pub layer: Option<String>,
    pub opaque: usize,
    pub charge: Lease,
}
fn bound(
    value: &PyObjectRef,
    depth: usize,
    ancestors: &mut [usize; 65],
    fuel: &mut usize,
) -> Result<usize, String> {
    if depth > 64 || ancestors[..depth].contains(&value.get_id()) {
        return Ok(0);
    }
    *fuel = fuel
        .checked_sub(1)
        .ok_or("Native Face preflight work admission refused")?;
    ancestors[depth] = value.get_id();
    let mut size = 128usize;
    if let Some(text) = value.downcast_ref::<PyStr>() {
        if let Some(text) = text.as_utf8() {
            size = size
                .checked_add(text.as_str().len())
                .ok_or("Native Face string extent overflow")?;
        }
    }
    if let Some(list) = value.downcast_ref::<PyList>() {
        for value in list.borrow_vec().iter() {
            size = size
                .checked_add(bound(value, depth + 1, ancestors, fuel)?)
                .ok_or("Native Face list extent overflow")?;
        }
    }
    if let Some(tuple) = value.downcast_ref::<PyTuple>() {
        for value in tuple.as_slice() {
            size = size
                .checked_add(bound(value, depth + 1, ancestors, fuel)?)
                .ok_or("Native Face tuple extent overflow")?;
        }
    }
    if let Some(dict) = value.downcast_ref::<PyDict>() {
        for (key, value) in dict {
            size = size
                .checked_add(bound(&key, depth + 1, ancestors, fuel)?)
                .and_then(|n| {
                    bound(&value, depth + 1, ancestors, fuel)
                        .ok()
                        .and_then(|m| n.checked_add(m))
                })
                .ok_or("Native Face dict extent overflow")?;
        }
    }
    Ok(size)
}
fn text(value: &PyObjectRef) -> Result<Option<String>, String> {
    let Some(value) = value.downcast_ref::<PyStr>() else {
        return Ok(None);
    };
    let Some(value) = value.as_utf8() else {
        return Ok(None);
    };
    let mut result = String::new();
    result
        .try_reserve_exact(value.as_str().len())
        .map_err(|_| "Native Face string allocation refused")?;
    result.push_str(value.as_str());
    Ok(Some(result))
}
fn project(
    value: &PyObjectRef,
    depth: usize,
    vm: &VirtualMachine,
    ancestors: &mut [usize; 65],
    fuel: &mut usize,
) -> Result<Option<Value>, String> {
    if depth > 64 || ancestors[..depth].contains(&value.get_id()) {
        return Ok(None);
    }
    *fuel = fuel
        .checked_sub(1)
        .ok_or("Native Face projection work admission refused")?;
    ancestors[depth] = value.get_id();
    if value.is(&vm.ctx.none()) {
        return Ok(Some(Value::Null));
    }
    if value.is(&vm.ctx.true_value) {
        return Ok(Some(Value::Bool(true)));
    }
    if value.is(&vm.ctx.false_value) {
        return Ok(Some(Value::Bool(false)));
    }
    if let Some(v) = value.downcast_ref::<PyInt>() {
        return Ok(v
            .as_bigint()
            .to_i64()
            .map(|v| Value::Number(v.into()))
            .or_else(|| v.as_bigint().to_u64().map(|v| Value::Number(v.into()))));
    }
    if let Some(v) = value.downcast_ref::<PyFloat>() {
        return Ok(serde_json::Number::from_f64(v.to_f64()).map(Value::Number));
    }
    if let Some(v) = text(value)? {
        return Ok(Some(Value::String(v)));
    }
    let mut array = |values: &[PyObjectRef]| -> Result<Option<Value>, String> {
        let mut out = Vec::new();
        out.try_reserve_exact(values.len())
            .map_err(|_| "Native Face array allocation refused")?;
        for v in values {
            let Some(v) = project(v, depth + 1, vm, ancestors, fuel)? else {
                return Ok(None);
            };
            out.push(v);
        }
        Ok(Some(Value::Array(out)))
    };
    if let Some(v) = value.downcast_ref::<PyList>() {
        return array(&v.borrow_vec());
    }
    if let Some(v) = value.downcast_ref::<PyTuple>() {
        return array(v.as_slice());
    }
    if let Some(v) = value.downcast_ref::<PyDict>() {
        let mut out = serde_json::Map::new();
        for (k, v) in v {
            let Some(k) = text(&k)? else { return Ok(None) };
            let Some(v) = project(&v, depth + 1, vm, ancestors, fuel)? else {
                return Ok(None);
            };
            out.insert(k, v);
        }
        return Ok(Some(Value::Object(out)));
    }
    Ok(None)
}
pub(super) fn prepare(
    attrs: &PyObjectRef,
    budget: &OwnerBudget,
    vm: &VirtualMachine,
) -> Result<Fields, String> {
    let dict = attrs
        .downcast_ref::<PyDict>()
        .ok_or("Native Face projection requires genuine dict")?;
    let mut charge = budget.reserve(
        Kind::Preparation,
        bound(attrs, 0, &mut [0; 65], &mut 1_000_000)?,
    )?;
    let mut out = HashMap::new();
    out.try_reserve(dict.__len__())
        .map_err(|_| "Native Face field table refused")?;
    let mut opaque = 0;
    let mut fuel = 1_000_000;
    let mut ancestors = [0; 65];
    for (key, value) in dict {
        let Some(key) = text(&key)? else {
            opaque += 1;
            continue;
        };
        let Some(value) = project(&value, 0, vm, &mut ancestors, &mut fuel)? else {
            opaque += 1;
            continue;
        };
        out.insert(key, value);
    }
    let layer = if let Some(value) = out.get("layer").and_then(Value::as_str) {
        charge.resize(
            charge
                .bytes()
                .checked_add(value.len())
                .ok_or("Native Face layer bytes overflow")?,
        )?;
        let mut layer = String::new();
        layer
            .try_reserve_exact(value.len())
            .map_err(|_| "Native Face layer allocation refused")?;
        layer.push_str(value);
        Some(layer)
    } else {
        None
    };
    Ok(Fields {
        attrs: out,
        layer,
        opaque,
        charge,
    })
}

/// Cold command comparison. Empty default attrs need no payload materialization.
/// This only inspects builtin payloads; arbitrary values remain VM-owned.
pub(super) fn needs_projection(
    mesh: &rustpython_vm::PyRef<super::editable_mesh::Mesh>,
    budget: &OwnerBudget,
    vm: &VirtualMachine,
) -> Result<bool, String> {
    let faces = mesh.faces.borrow_vec();
    let owner = mesh.owner.read();
    if faces.len() != owner.mesh().faces.len() {
        return Ok(false);
    }
    for (value, native) in faces.iter().zip(&owner.mesh().faces) {
        let face = value
            .downcast_ref::<super::editable_mesh::Face>()
            .ok_or("Native Face field identity requires Face")?;
        let attrs = face.attrs.read().clone();
        if attrs
            .downcast_ref::<PyDict>()
            .is_some_and(|dict| dict.__len__() == 0)
            && native.attrs.is_empty()
            && native.layer.is_none()
        {
            continue;
        }
        let fields = prepare(&attrs, budget, vm)?;
        if fields.attrs != native.attrs || fields.layer != native.layer {
            return Ok(true);
        }
    }
    Ok(false)
}
