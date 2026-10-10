use rustpython_vm::{InterpreterBuilder,compiler::Mode,pymodule,VirtualMachine,PyResult,scope::Scope};
#[pymodule]
mod native_probe {
    #[pyfunction]
    fn add(a:i32,b:i32)->i32 {a+b}
}
fn execute(vm:&VirtualMachine,scope:&Scope,source:&str)->PyResult<()> {
 match vm.compile(source,Mode::Eval,"<ai>") {
  Ok(code)=>{
   let value=vm.run_code_obj(code,scope.clone())?;
   if !vm.is_none(&value){
    scope.globals.set_item("_",value,vm)?;
    vm.run_code_obj(vm.compile("print(repr(_))",Mode::Exec,"<ai>").unwrap(),scope.clone())?;
   }
  },
  Err(_)=>{
   let code=vm.compile(source,Mode::Exec,"<ai>").map_err(|error|error.into_pyexception(vm,Some(source)))?;
   vm.run_code_obj(code,scope.clone())?;
  },
 }
 Ok(())
}
fn main(){
 let builder=InterpreterBuilder::new();
 let module=native_probe::module_def(&builder.ctx);
 let interpreter=builder.add_native_module(module).interpreter();
 interpreter.enter(|vm|{
  let scope=vm.new_scope_with_builtins();
  let setup=r#"import sys
import native_probe
class Capture:
    def __init__(self): self.parts=[]
    def write(self,text): self.parts.append(text)
    def flush(self): pass
out=Capture()
err=Capture()
sys.stdout=out
sys.stderr=err
"#;
  let code=vm.compile(setup,Mode::Exec,"<ai>").expect("genuine exec compilation");
  vm.run_code_obj(code,scope.clone()).expect("genuine classes/import/loop/native module execution");
  execute(vm,&scope,include_str!("../language.py")).unwrap();
  execute(vm,&scope,"counter.twice()").unwrap();
  execute(vm,&scope,"None").unwrap();
  execute(vm,&scope,"assert _ == 570\ncounter.value += 1\nassert counter.twice()==572").unwrap();
  let syntax_refused=execute(vm,&scope,"def broken(").is_err();
  let exception=execute(vm,&scope,"raise ValueError('literal-failure')").unwrap_err();
  let mut traceback=String::new();vm.write_exception(&mut traceback,&exception).unwrap();
  assert!(syntax_refused&&traceback.contains("ValueError: literal-failure"));
  let actual_stdout=vm.run_code_obj(vm.compile("''.join(out.parts)",Mode::Eval,"<ai>").unwrap(),scope.clone()).unwrap().str(vm).unwrap().to_str().unwrap().to_owned();
  let actual_value=vm.run_code_obj(vm.compile("counter.twice()",Mode::Eval,"<ai>").unwrap(),scope).unwrap().repr(vm).unwrap().to_str().unwrap().to_owned();
  println!("{}",serde_json::json!({"engine":"RustPython0.6.0","persistent_scope":true,"functions_classes_loops":true,"eval_exec":true,"native_module":true,"captured_stdout":actual_stdout,"persistent_value":actual_value,"traceback":traceback,"syntax_refused":syntax_refused,"runtime_exception":true,"scene_adapters":false,"atomic_journal":false}));
 });
}
