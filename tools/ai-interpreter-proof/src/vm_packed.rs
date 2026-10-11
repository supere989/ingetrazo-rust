//! Identity-owned packed closure: public membership first, then held topology/registry.
//! Public Vertex membership and source position snapshots are distinct from GPU rows.
use super::{editable_mesh::{Mesh,Vertex},FaceRestore,VertexRestore,GraphState};
use crate::{owner_budget::{OwnerBudget,Lease,Kind},journal::SourceKey};
use rustpython_vm::{PyRef,PyObjectRef,AsObject,builtins::{PyList,PyTuple,PyInt},VirtualMachine};
use std::collections::HashMap;
pub(super) struct Candidate{
 pub mesh:ingetrazo_core::mesh::Mesh,
 pub keys:Vec<(SourceKey,usize)>,
 assignments:Vec<(PyRef<Vertex>,uuid::Uuid)>,
 pub charge:Lease,
}
impl Candidate{
 pub fn apply(self,live:&PyRef<Mesh>,state:uuid::Uuid)->Result<(),String>{
  let Self{mesh,keys,assignments,charge}=self;
  let result={let mut owner=live.owner.write();owner.prepare_identity_replacement_as(mesh,keys,state).map(|ticket|ticket.apply())};
  let old=result?;for(vertex,uid)in assignments{vertex.packed_uid.set(Some(uid));}drop(old);drop(charge);Ok(())
 }
}
pub(super) fn prepare(mesh:&PyRef<Mesh>,public:&[PyObjectRef],graph:&GraphState,positions:&[VertexRestore],faces:&[PyObjectRef],fields:&[FaceRestore],budget:&OwnerBudget,vm:&VirtualMachine)->Result<Candidate,String>{
 if faces.len()!=fields.len(){return Err("Packed Face snapshot extent mismatch".into());}
 let registry=graph.registry.as_ref().ok_or("Missing public registry snapshot")?;
 let mut bound=public.len().checked_add(graph.members.len().checked_mul(2).ok_or("Packed endpoint overflow")?).and_then(|n|n.checked_add(registry.__len__())).ok_or("Packed closure extent overflow")?;
 for field in fields{
  bound=bound.checked_add(field.outer.borrow_vec().len()).ok_or("Packed ring extent overflow")?;
  let holes=field.holes.borrow_vec();
  for ring in holes.iter(){let ring=ring.downcast_ref::<PyList>().ok_or("Packed hole requires list")?;bound=bound.checked_add(ring.borrow_vec().len()).ok_or("Packed hole extent overflow")?;}
 }
 let mut charge=budget.reserve(Kind::Preparation,bound.checked_mul(256).and_then(|n|fields.len().checked_mul(128).and_then(|m|n.checked_add(m))).ok_or("Packed preparation byte overflow")?)?;
 let mut vertices=Vec::new();vertices.try_reserve_exact(bound).map_err(|_|"Packed closure references refused")?;
 let mut ids=HashMap::new();ids.try_reserve(bound).map_err(|_|"Packed closure identity map refused")?;
 fn add(value:PyObjectRef,ids:&mut HashMap<usize,usize>,vertices:&mut Vec<PyObjectRef>)->Result<(),String>{
  if value.downcast_ref::<Vertex>().is_none(){return Err("Packed closure requires genuine Vertex".into());}
  if !ids.contains_key(&value.get_id()){let id=vertices.len();ids.insert(value.get_id(),id);vertices.push(value);}
  Ok(())
 }
 for value in public{add(value.clone(),&mut ids,&mut vertices)?;}
 if vertices.len()!=public.len(){return Err("Repeated public Vertex membership unsupported".into());}
 for value in &graph.members{
  let edge=value.downcast_ref::<super::editable_mesh::Edge>().ok_or("Packed graph member requires Edge")?;
  let a={edge.v0.read().clone().ok_or("Cleared packed endpoint")?};let b={edge.v1.read().clone().ok_or("Cleared packed endpoint")?};add(a.into(),&mut ids,&mut vertices)?;add(b.into(),&mut ids,&mut vertices)?;
 }
 for field in fields{
  for value in field.outer.borrow_vec().iter(){add(value.clone(),&mut ids,&mut vertices)?;}
  for hole in field.holes.borrow_vec().iter(){let ring=hole.downcast_ref::<PyList>().ok_or("Packed hole requires list")?;for value in ring.borrow_vec().iter(){add(value.clone(),&mut ids,&mut vertices)?;}}
 }
 for(_,value)in registry{if value.downcast_ref::<Vertex>().is_some(){add(value,&mut ids,&mut vertices)?;}}
 // Public positions are command-owned; detached positions remain current, as
 // source snapshots only include public mesh.vertices, not referenced orphans.
 let mut old_positions=HashMap::new();old_positions.try_reserve(positions.len()).map_err(|_|"Packed snapshot position map refused")?;
 for p in positions{old_positions.insert(p.vertex.get_id(),p.position.coordinates());}
 let mut assignments=Vec::new();assignments.try_reserve_exact(vertices.len()).map_err(|_|"Packed UID assignments refused")?;
 let mut out=ingetrazo_core::mesh::Mesh::new();out.vertices.try_reserve_exact(vertices.len()).map_err(|_|"Packed Vertex records refused")?;
 for value in &vertices{
  let vertex=value.clone().downcast::<Vertex>().map_err(|_|"Packed Vertex identity invalid")?;
  let p=old_positions.get(&vertex.get_id()).copied().unwrap_or_else(||vertex.position.read().coordinates());
  let position=glam::DVec3::from_array(p.map(f64::from));if !position.is_finite(){return Err("Nonfinite packed Vertex".into());}
  let uid=vertex.packed_uid.get().unwrap_or_else(uuid::Uuid::new_v4);
  out.vertices.push(ingetrazo_core::mesh::MeshVertex{uid,position,edges:Vec::new()});assignments.push((vertex,uid));
 }
 let (edges,edge_charge)=graph.packed_edges(&vertices,faces,budget,vm)?;charge.merge(edge_charge)?;
 out.faces.try_reserve_exact(fields.len()).map_err(|_|"Packed Face records refused")?;
 let ring=|values:&rustpython_vm::builtins::PyListRef|->Result<Vec<usize>,String>{let values=values.borrow_vec();let mut result=Vec::new();result.try_reserve_exact(values.len()).map_err(|_|"Packed ring indices refused")?;for value in values.iter(){result.push(*ids.get(&value.get_id()).ok_or("Packed ring Vertex missing")?);}Ok(result)};
 for field in fields{
  let outer=ring(&field.outer)?;let holes=field.holes.borrow_vec();let mut rings=Vec::new();rings.try_reserve_exact(holes.len()).map_err(|_|"Packed hole slots refused")?;
  for hole in holes.iter(){rings.push(ring(&hole.clone().downcast::<PyList>().map_err(|_|"Packed hole requires list")?)?);}
  let mut face=ingetrazo_core::mesh::MeshFace::new(outer);face.holes=rings;out.faces.push(face);
 }
 let mut counts=Vec::new();counts.try_reserve_exact(vertices.len()).map_err(|_|"Packed incidence counts refused")?;counts.resize(vertices.len(),0usize);
 for edge in &edges{counts[edge.v0]=counts[edge.v0].checked_add(1).ok_or("Packed incidence overflow")?;if edge.v1!=edge.v0{counts[edge.v1]=counts[edge.v1].checked_add(1).ok_or("Packed incidence overflow")?;}}
 let links=counts.iter().try_fold(0usize,|n,&m|n.checked_add(m)).ok_or("Packed link extent overflow")?;charge.resize(charge.bytes().checked_add(links.checked_mul(std::mem::size_of::<usize>()).ok_or("Packed link bytes overflow")?).ok_or("Packed link charge overflow")?)?;
 for(vertex,count)in out.vertices.iter_mut().zip(counts){vertex.edges.try_reserve_exact(count).map_err(|_|"Packed incidence allocation refused")?;}
 for(id,edge)in edges.iter().enumerate(){out.vertices[edge.v0].edges.push(id);if edge.v0!=edge.v1{out.vertices[edge.v1].edges.push(id);}}
 out.edges=edges;
 let mut keys=Vec::new();keys.try_reserve_exact(registry.__len__()).map_err(|_|"Packed registry rows refused")?;
 for(key,value)in registry{
  // The actual dictionary is retained verbatim. This derived numeric cache
  // admits only literal integer-cell -> Vertex pairs; other Python entries
  // remain in VM/history and input lookup admission detects cache disagreement.
  let Some(tuple)=key.downcast_ref::<PyTuple>() else{continue};
  if tuple.as_slice().len()!=3||tuple.as_slice().iter().any(|c|c.downcast_ref::<PyInt>().is_none())||value.downcast_ref::<Vertex>().is_none(){continue;}
  let mut cells=Vec::new();cells.try_reserve_exact(3).map_err(|_|"Packed registry cells refused")?;
  for cell in tuple.as_slice(){let n=cell.downcast_ref::<PyInt>().unwrap();cells.push(n.as_bigint().to_string().parse::<num_bigint::BigInt>().map_err(|_|"Packed registry integer conversion refused")?);}
  let id=*ids.get(&value.get_id()).ok_or("Native registry Vertex closure missing")?;
  keys.push((SourceKey(cells.try_into().map_err(|_|"Packed registry cell extent")?),id));
 }
 let native=mesh.owner.read();if native.mesh().faces.iter().any(|face|!face.attrs.is_empty()||face.layer.is_some()||face.interior){return Err("Native painted Face closure admission pending".into());}
 Ok(Candidate{mesh:out,keys,assignments,charge})
}
