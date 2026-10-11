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

fn main(){
 let mut c=vm_mesh::Controller::new();c.enable_history();
 c.assert_script("m=mesh\nf=m.add_face([QVector3D(),QVector3D(1,0,0),QVector3D(0,1,0)])\nr=m._registry\nv=m.vertices[0]\nassert r is m._registry\nassert r[(0,0,0)] is v\nassert m.vertex_at(QVector3D()) is v");
 c.assert_script("r[(90000,0,0)]=v\nassert m._lookup(QVector3D(9,0,0)) is v");
 assert!(c.undo().unwrap());c.assert_script("assert r is m._registry\nassert (90000,0,0) not in r\nassert r[(0,0,0)] is v");
 assert!(c.redo().unwrap());c.assert_script("assert r is m._registry\nassert r[(90000,0,0)] is v");
 assert!(c.execute("r.clear()\nraise ValueError('restore')").is_err());
 c.assert_script("assert r is m._registry\nassert r[(0,0,0)] is v\nassert r[(90000,0,0)] is v");
 assert!(c.execute("m.add_face([QVector3D(9,0,0),QVector3D(10,0,0),QVector3D(9,1,0)])").is_err());
 c.assert_script("assert r[(90000,0,0)] is v\nassert len(m.faces)==1");
 let b=c.budget();drop(c);assert_eq!(b.snapshot().total(),0);assert_eq!(vm_mesh::graph_payload_lifetimes(),(0,0));
 println!("{}",serde_json::json!({"persistent_dictionary":true,"undo_redo_owned_contents":true,"failed_command_restore":true,"final_charge":0,"production_enabled":false}));
}
