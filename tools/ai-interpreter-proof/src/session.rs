use rustpython_vm::{VirtualMachine,PyResult,scope::Scope,compiler::Mode};
pub fn execute(vm:&VirtualMachine,scope:&Scope,source:&str)->PyResult<()> {
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
