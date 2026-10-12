use rustpython_vm::{PyResult, VirtualMachine, compiler::Mode, scope::Scope};
pub fn execute(vm: &VirtualMachine, scope: &Scope, source: &str) -> PyResult<()> {
    match vm.compile(source, Mode::Eval, "<ai>") {
        Ok(code) => {
            let value = vm.run_code_obj(code, scope.clone())?;
            if !vm.is_none(&value) {
                scope.globals.set_item("_", value, vm)?;
                vm.run_code_obj(
                    vm.compile("print(repr(_))", Mode::Exec, "<ai>").unwrap(),
                    scope.clone(),
                )?;
            }
        }
        Err(_) => {
            let code = vm
                .compile(source, Mode::Exec, "<ai>")
                .map_err(|error| error.into_pyexception(vm, Some(source)))?;
            vm.run_code_obj(code, scope.clone())?;
        }
    }
    Ok(())
}

/// Borrow the code supplied by the actual protocol, without converting it to source text.
#[derive(Clone, Copy)]
pub enum CodeInput<'a> {
    Text(&'a str),
    Json(&'a serde_json::Value),
}
fn json_object(
    vm: &VirtualMachine,
    value: &serde_json::Value,
) -> PyResult<rustpython_vm::PyObjectRef> {
    vm.host_budget_tick()?;
    Ok(match value {
        serde_json::Value::Null => vm.ctx.none(),
        serde_json::Value::Bool(value) => vm.ctx.new_bool(*value).into(),
        serde_json::Value::String(value) => vm.ctx.new_str(value.clone()).into(),
        serde_json::Value::Number(value) => {
            if let Some(value) = value.as_i64() {
                vm.ctx.new_int(value).into()
            } else if let Some(value) = value.as_u64() {
                vm.ctx.new_int(value).into()
            } else {
                vm.ctx
                    .new_float(
                        value
                            .as_f64()
                            .ok_or_else(|| vm.new_value_error("JSON code number refused"))?,
                    )
                    .into()
            }
        }
        serde_json::Value::Array(values) => {
            let mut objects = Vec::new();
            objects
                .try_reserve_exact(values.len())
                .map_err(|_| vm.new_memory_error("JSON code argument admission refused"))?;
            for value in values {
                objects.push(json_object(vm, value)?);
            }
            vm.ctx.new_list(objects).into()
        }
        serde_json::Value::Object(values) => {
            let dict = vm.ctx.new_dict();
            for (key, value) in values {
                dict.set_item(key, json_object(vm, value)?, vm)?;
            }
            dict.into()
        }
    })
}
fn execute_input(vm: &VirtualMachine, scope: &Scope, input: CodeInput<'_>) -> PyResult<()> {
    match input {
        CodeInput::Text(source) => execute(vm, scope, source),
        CodeInput::Json(serde_json::Value::String(source)) => execute(vm, scope, source),
        CodeInput::Json(value) => {
            // Let the genuine builtin compiler reject the supplied object. No Python
            // error text, traceback, or changed result is supplied by the bridge.
            let compile = vm.builtins.get_attr("compile", vm)?;
            let source = json_object(vm, value)?;
            vm.invoke(
                &compile,
                (source, vm.ctx.new_str("<ai>"), vm.ctx.new_str("eval")),
            )?;
            Err(vm.new_runtime_error("Nonstring JSON code unexpectedly compiled"))
        }
    }
}

/// Actual transcript from one genuine VM execution, before native publication.
#[derive(Default, Debug)]
pub struct ExecutionReceipt {
    pub stdout: String,
    pub stderr: String,
    pub error: Option<String>,
    pub changed: bool,
}

#[rustpython_vm::pymodule]
pub(crate) mod transcript {
    use rustpython_vm::{Py, PyPayload, PyResult, VirtualMachine, common::lock::PyRwLock, pyclass};
    #[pyattr]
    #[pyclass(name = "Stream")]
    #[derive(Debug, PyPayload)]
    pub struct Stream {
        pub(crate) text: PyRwLock<String>,
        pub(crate) refused: std::sync::atomic::AtomicBool,
    }
    #[pyclass]
    impl Stream {
        #[pymethod]
        fn write(zelf: &Py<Self>, text: String, vm: &VirtualMachine) -> PyResult<usize> {
            vm.host_budget_tick()?;
            let mut out = zelf.text.write();
            if out
                .len()
                .checked_add(text.len())
                .is_none_or(|n| n > 1024 * 1024)
                || out.try_reserve(text.len()).is_err()
            {
                zelf.refused
                    .store(true, std::sync::atomic::Ordering::Relaxed);
                vm.host_budget_abort();
                return Err(vm.new_memory_error("Bridge transcript admission refused"));
            }
            out.push_str(&text);
            Ok(text.chars().count())
        }
        #[pymethod]
        fn flush(_zelf: &Py<Self>) {}
        #[pymethod]
        fn isatty(_zelf: &Py<Self>) -> bool {
            false
        }
        #[pymethod]
        fn writable(_zelf: &Py<Self>) -> bool {
            true
        }
        #[pygetset]
        fn encoding(_zelf: &Py<Self>) -> Option<String> {
            None
        }
    }
    impl Stream {
        pub(crate) fn new() -> Self {
            Self {
                text: PyRwLock::new(String::new()),
                refused: std::sync::atomic::AtomicBool::new(false),
            }
        }
    }
}

struct Streams<'a> {
    vm: &'a VirtualMachine,
    stdout: rustpython_vm::PyObjectRef,
    stderr: rustpython_vm::PyObjectRef,
}
impl Drop for Streams<'_> {
    fn drop(&mut self) {
        // Restore actual module slots even after a script overwrites sys streams.
        self.vm
            .sys_module
            .set_attr("stdout", self.stdout.clone(), self.vm)
            .expect("sys stdout restore");
        self.vm
            .sys_module
            .set_attr("stderr", self.stderr.clone(), self.vm)
            .expect("sys stderr restore");
    }
}

pub fn execute_captured(
    vm: &VirtualMachine,
    scope: &Scope,
    source: CodeInput<'_>,
) -> Result<(PyResult<()>, ExecutionReceipt), String> {
    use rustpython_vm::PyPayload;
    let streams = Streams {
        vm,
        stdout: vm
            .sys_module
            .get_attr("stdout", vm)
            .map_err(|_| "stdout capture admission refused")?,
        stderr: vm
            .sys_module
            .get_attr("stderr", vm)
            .map_err(|_| "stderr capture admission refused")?,
    };
    let stdout = transcript::Stream::new().into_ref(&vm.ctx);
    let stderr = transcript::Stream::new().into_ref(&vm.ctx);
    vm.sys_module
        .set_attr("stdout", stdout.clone(), vm)
        .map_err(|_| "stdout capture admission refused")?;
    vm.sys_module
        .set_attr("stderr", stderr.clone(), vm)
        .map_err(|_| "stderr capture admission refused")?;
    let outcome = execute_input(vm, scope, source);
    let mut receipt = ExecutionReceipt::default();
    if let Err(error) = &outcome {
        use rustpython_vm::AsObject;
        let message = error
            .as_object()
            .str(vm)
            .map_err(|_| "exception message refused")?;
        let message = message
            .to_str()
            .ok_or("Exception message contains unsupported non-UTF8 codepoints")?;
        if message.len() > 1024 * 1024 {
            vm.host_budget_abort();
            return Err("Exception message admission refused".into());
        }
        receipt.error = Some(format!("SnapshotImport: {message}"));
        // Format the actual interpreter exception directly through the bounded stream.
        // No unbounded temporary traceback String and no console-log inference.
        let mut writer = rustpython_vm::py_io::PyWriter(stderr.clone().into(), vm);
        vm.write_exception(&mut writer, error)
            .map_err(|_| "exception transcript refused")?;
    }
    if stdout.refused.load(std::sync::atomic::Ordering::Relaxed)
        || stderr.refused.load(std::sync::atomic::Ordering::Relaxed)
    {
        vm.host_budget_abort();
        return Err("Bridge transcript admission refused".into());
    }
    receipt.stdout = std::mem::take(&mut *stdout.text.write());
    receipt.stderr = std::mem::take(&mut *stderr.text.write());
    drop(streams);
    Ok((outcome, receipt))
}
