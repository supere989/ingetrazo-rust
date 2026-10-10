use std::cell::RefCell;
use ingetrazo_core::scene::Scene;
use glam::DVec3;
use rustpython_vm::{InterpreterBuilder,pymodule,VirtualMachine,PyResult,compiler::Mode};
#[path="../journal.rs"]mod journal;
#[path="../session.rs"]mod session;
thread_local!{static ACTIVE:RefCell<Option<journal::MeshJournal>>=const{RefCell::new(None)};}
#[pymodule]mod native_scene {
 use super::*;
 #[pyfunction]fn add_edge(ax:f64,ay:f64,az:f64,bx:f64,by:f64,bz:f64,vm:&VirtualMachine)->PyResult<(usize,f64,f64,f64,f64,f64,f64)> {
  ACTIVE.with(|active|active.borrow_mut().as_mut().ok_or("No active transaction".to_owned()).and_then(|j|j.add_edge(DVec3::new(ax,ay,az),DVec3::new(bx,by,bz))).map(|(id,a,b)|(id,a.x,a.y,a.z,b.x,b.y,b.z)).map_err(|error|vm.new_value_error(error)))
 }
}
fn main(){
 let builder=InterpreterBuilder::new();let module=native_scene::module_def(&builder.ctx);
 let interpreter=builder.add_native_module(module).interpreter();let mut scene=Scene::new();let mut results=vec![];
 interpreter.enter(|vm|{
  let scope=vm.new_scope_with_builtins();
  let adapter=r#"import sys
import native_scene
class Capture:
    def __init__(self): self.parts=[]
    def write(self,text): self.parts.append(text)
    def flush(self): pass
out=Capture()
sys.stdout=out
class QVector3D:
    def __init__(self,x,y,z): self.x=float(x);self.y=float(y);self.z=float(z)
class Edge:
    def __init__(self,coords): self.coords=coords
    def __repr__(self):
        ax,ay,az,bx,by,bz=self.coords
        return f'Edge(Vertex({ax:.3f}, {ay:.3f}, {az:.3f}) -> Vertex({bx:.3f}, {by:.3f}, {bz:.3f}))'
_native_edges={}
class Mesh:
    def add_edge(self,a,b):
        id,*coords=native_scene.add_edge(a.x,a.y,a.z,b.x,b.y,b.z)
        if id not in _native_edges: _native_edges[id]=Edge(coords)
        return _native_edges[id]
mesh=Mesh()
"#;
  vm.run_code_obj(vm.compile(adapter,Mode::Exec,"<native-adapter>").unwrap(),scope.clone()).unwrap();
  let cases:serde_json::Value=serde_json::from_str(include_str!("../../transaction-cases.json")).unwrap();
  for case in cases.as_array().unwrap(){
   ACTIVE.with(|active|*active.borrow_mut()=Some(journal::MeshJournal::begin(&scene.mesh,scene.version)));
   vm.run_code_obj(vm.compile("out.parts.clear()",Mode::Exec,"<capture>").unwrap(),scope.clone()).unwrap();
   let error=session::execute(vm,&scope,case["code"].as_str().unwrap()).err();
   let delta=ACTIVE.with(|active|active.borrow_mut().take().unwrap());
   let staged_vertices=delta.added_vertices.len();let staged_edges=delta.added_edges.len();
   if error.is_some(){
    // Pending edge objects can stay alive through Python scope references, but
    // never stay indexed as live scene objects or reattach on index reuse.
    let cleanup=format!("for key in range({}, {}):\n    _native_edges.pop(key, None)",delta.base_edges,delta.base_edges+staged_edges);
    vm.run_code_obj(vm.compile(&cleanup,Mode::Exec,"<discard-handles>").unwrap(),scope.clone()).unwrap();
   }
   let changed=if error.is_none(){let changed=delta.publish(&mut scene.mesh,scene.version).unwrap();if changed{scene.bump_version();}changed}else{false};
   let stdout=vm.run_code_obj(vm.compile("''.join(out.parts)",Mode::Eval,"<capture>").unwrap(),scope.clone()).unwrap().str(vm).unwrap().to_str().unwrap().to_owned();
   let mut traceback=String::new();if let Some(error)=&error{vm.write_exception(&mut traceback,error).unwrap();}
   results.push(serde_json::json!({"name":case["name"],"changed":changed,"failed":error.is_some(),"stdout":stdout,"vertices":scene.mesh.vertices.iter().map(|v|v.position.to_array()).collect::<Vec<_>>(),"edges":scene.mesh.edges.iter().map(|e|[e.v0,e.v1]).collect::<Vec<_>>(),"staged_vertices":staged_vertices,"staged_edges":staged_edges,"traceback":traceback}));
  }
 });
 println!("{}",serde_json::json!({"cases":results,"scope":"append-only unpainted mesh; no production integration","scene_clones_per_edge":0}));
}
