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
        fn flush(&self) {}
        #[pymethod]
        fn isatty(&self) -> bool {
            false
        }
        #[pymethod]
        fn writable(&self) -> bool {
            true
        }
        #[pygetset]
        fn encoding(&self) -> Option<String> {
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
    source: &str,
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
    let outcome = execute(vm, scope, source);
    let mut receipt = ExecutionReceipt::default();
    if let Err(error) = &outcome {
        use rustpython_vm::AsObject;
        let message = error
            .as_object()
            .str(vm)
            .map_err(|_| "exception message refused")?;
        if message.to_str().len() > 1024 * 1024 {
            vm.host_budget_abort();
            return Err("Exception message admission refused".into());
        }
        receipt.error = Some(format!("SnapshotImport: {}", message.to_str()));
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
