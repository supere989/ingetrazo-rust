#[path = "../face_owner.rs"]
mod face_owner;
#[path = "../journal.rs"]
mod journal;
#[path = "../owner.rs"]
mod owner;
#[path = "../owner_budget.rs"]
mod owner_budget;
#[path = "../owner_registry.rs"]
mod owner_registry;
#[path = "../session.rs"]
mod session;
#[path = "../source_holes.rs"]
mod source_holes;
#[path = "../vm_groups.rs"]
mod vm_groups;
#[path = "../vm_mesh.rs"]
mod vm_mesh;

use rustpython_vm::AsObject;
use glam::DVec3;
use std::{alloc::{GlobalAlloc,Layout,System},sync::atomic::{AtomicBool,Ordering}};
static REFUSE_MAP:AtomicBool=AtomicBool::new(false);
struct Alloc;
unsafe impl GlobalAlloc for Alloc{
 unsafe fn alloc(&self,l:Layout)->*mut u8{if REFUSE_MAP.swap(false,Ordering::SeqCst){return std::ptr::null_mut();}unsafe{System.alloc(l)}}
 unsafe fn alloc_zeroed(&self,l:Layout)->*mut u8{if REFUSE_MAP.swap(false,Ordering::SeqCst){return std::ptr::null_mut();}unsafe{System.alloc_zeroed(l)}}
 unsafe fn realloc(&self,p:*mut u8,l:Layout,n:usize)->*mut u8{if REFUSE_MAP.swap(false,Ordering::SeqCst){return std::ptr::null_mut();}unsafe{System.realloc(p,l,n)}}
 unsafe fn dealloc(&self,p:*mut u8,l:Layout){unsafe{System.dealloc(p,l)}}
}
#[global_allocator]static ALLOC:Alloc=Alloc;
fn main(){
 let mut c=vm_mesh::Controller::new();c.assert_script("m=mesh\nf=m.add_face([QVector3D(),QVector3D(1,0,0),QVector3D(0,1,0)])\nv0=m.vertices[0]\nv1=m.vertices[1]");
 let a=c.held_vertex("v0");let b=c.held_vertex("v1");let budget=c.budget();
 let mut r=c.identity_registry().unwrap();
 let rows=[(DVec3::ZERO,a.clone()),(DVec3::X,b.clone())];
 let prepared=r.prepare(&rows).unwrap();assert_eq!(r.len(),0);let held=budget.snapshot().preparation;assert!(held>0);drop(prepared);assert_eq!(budget.snapshot().preparation,0);
 let prepared=r.prepare(&rows).unwrap();drop(r.publish(prepared).unwrap());assert!(r.slot(DVec3::ZERO).unwrap().unwrap().same(&a));
 let old=r.prepare(&[(DVec3::ZERO,b.clone())]).unwrap();let current=r.prepare(&rows).unwrap();drop(r.publish(current).unwrap());assert!(r.publish(old).is_err());assert!(r.slot(DVec3::ZERO).unwrap().unwrap().same(&a));
 let before=budget.snapshot().total();budget.set_limit(Some(before));assert!(r.prepare(&rows).is_err());assert_eq!(budget.snapshot().total(),before);assert!(r.slot(DVec3::ZERO).unwrap().unwrap().same(&a));budget.set_limit(None);
 let before=budget.snapshot().total();let revision=r.revision();REFUSE_MAP.store(true,Ordering::SeqCst);assert!(r.prepare(&rows).is_err());assert!(!REFUSE_MAP.load(Ordering::SeqCst));assert_eq!(budget.snapshot().total(),before);assert_eq!(r.revision(),revision);assert!(r.slot(DVec3::ZERO).unwrap().unwrap().same(&a));
 let foreign=c.identity_registry().unwrap();let candidate=foreign.prepare(&rows).unwrap();assert!(r.publish(candidate).is_err());drop(foreign);
 c.assert_script("v0.position.setX(5)");
 assert!(r.slot(DVec3::ZERO).unwrap().unwrap().same(&a));assert!(r.slot(DVec3::new(5.,0.,0.)).unwrap().is_none());
 // A foreign prepared candidate is the last root for a genuine native
 // finalizer. Rejection must RETURN ownership, never run it under owner borrow.
 let mut retiring=vm_mesh::Controller::new();let borrowed=std::sync::Arc::new(AtomicBool::new(false));let(finalized,inside)=retiring.proof_native_finalizer(borrowed.clone());
 retiring.assert_script("class Final:\n    __del__=native_finalize\nu=Vertex(QVector3D())\nu.edges=Final()\nu.edges.owner=u");let v=retiring.held_vertex("u");let foreign=retiring.identity_registry().unwrap();let before=budget.snapshot().total();assert!(r.prepare(&[(DVec3::ZERO,v.clone())]).is_err());assert_eq!(budget.snapshot().total(),before);let candidate=foreign.prepare(&[(DVec3::ZERO,v.clone())]).unwrap();drop(foreign);drop(retiring);drop(v);
 assert_eq!(finalized.load(Ordering::SeqCst),0);assert_eq!(vm_mesh::graph_payload_lifetimes(),(4,3));let cell=std::cell::RefCell::new(r);let mut owner_borrow=cell.borrow_mut();borrowed.store(true,Ordering::SeqCst);let rejection=owner_borrow.publish(candidate).err().expect("Foreign replacement must refuse");assert!(rejection.reason.contains("Foreign"));assert_eq!(finalized.load(Ordering::SeqCst),0);assert_eq!(vm_mesh::graph_payload_lifetimes(),(4,3));drop(owner_borrow);borrowed.store(false,Ordering::SeqCst);drop(rejection);r=cell.into_inner();assert_eq!(finalized.load(Ordering::SeqCst),1);assert_eq!(inside.load(Ordering::SeqCst),0);assert_eq!(vm_mesh::graph_payload_lifetimes(),(3,3));
 // Public membership/controller teardown cannot change the registry's held identity.
 drop(c);assert!(r.contains_identity(&a));assert!(r.slot(DVec3::X).unwrap().unwrap().same(&b));
 let candidate=r.prepare(&[(DVec3::ZERO,a.clone()),(DVec3::ZERO,b.clone())]).unwrap();let old=r.publish(candidate).unwrap();assert_eq!(r.len(),1);assert!(r.slot(DVec3::ZERO).unwrap().unwrap().same(&b));assert!(r.contains_identity(&b));drop(old);
 let candidate=r.prepare(&[]).unwrap();let old=r.publish(candidate).unwrap();assert_eq!(r.len(),0);assert!(budget.snapshot().registry>0);drop(old);drop(r);assert_eq!(budget.snapshot().total(),0);drop(a);drop(b);assert_eq!(vm_mesh::graph_payload_lifetimes(),(3,3));drop(rows);assert_eq!(vm_mesh::graph_payload_lifetimes(),(0,0));
 println!("{}",serde_json::json!({"genuine_vertex_identity":true,"detached_controller_lifetime":true,"duplicate_key_last_wins":true,"raw_move_preserves_old_key":true,"stale_and_foreign_refused":true,"foreign_vm_refused":true,"external_handles_keep_payloads_alive":[3,3],"prepare_discard_charge_released":true,"quota_refusal_preserves_registry":true,"rejected_candidate_finalizes_only_after_unlocked_drop":true,"native_finalizer_calls":1,"native_finalizer_under_owner_borrow":0,"actual_map_allocation_refusals":1,"actual_graph_payloads_final":[0,0],"final_ledger":0,"source_weld_lookup_complete":false,"native_replacement_complete":false,"production_enabled":false}));
}
