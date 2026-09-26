use std::{path::Path, sync::Arc, time::Instant};

use wasmtime::{Linker, Module, Store, TypedFunc};
use wasmtime_wasi::p2::pipe::MemoryOutputPipe;
use wasmtime_wasi::{FsPerms, I32Exit, WasiCtxBuilder, p1};

use crate::{
    Host, gpu, precompile::test_module, shared_memory, state::State, threads, transport, wake,
};

const START: &str = "_start";
const LISTING_CAPACITY: usize = 1 << 20;
const CARRIED: [&str; 2] = ["--ignored", "--include-ignored"];

impl Host {
    pub fn run_tests(
        &self,
        wasm: &Path,
        arguments: &[String],
        root: &Path,
        precompiled: Option<&Path>,
    ) -> Result<i32, String> {
        let module = test_module(&self.engine, wasm, precompiled)?;
        match self.run_instance(&module, arguments, root, None) {
            Ok(0) => return Ok(0),
            Ok(_) => {}
            Err(failure) => eprintln!("{failure}"),
        }
        let names = self.list_tests(&module, arguments, root)?;
        eprintln!(
            "a panic ends a wasm test run, so the {} tests run again one at a time",
            names.len()
        );
        let carried: Vec<&String> = arguments
            .iter()
            .skip(1)
            .filter(|argument| CARRIED.contains(&argument.as_str()))
            .collect();
        let mut failed = Vec::new();
        for name in names {
            let mut single = vec![arguments[0].clone(), name.clone(), "--exact".to_owned()];
            single.extend(carried.iter().map(|argument| (*argument).clone()));
            single.extend(["--test-threads=1".to_owned(), "--nocapture".to_owned()]);
            match self.run_instance(&module, &single, root, None) {
                Ok(0) => {}
                Ok(_) => failed.push(name),
                Err(failure) => {
                    eprintln!("{failure}");
                    failed.push(name);
                }
            }
        }
        match failed.is_empty() {
            true => eprintln!("every test passed on its own, but not in one run together"),
            false => {
                eprintln!("{} tests failed:", failed.len());
                for name in &failed {
                    eprintln!("    {name}");
                }
            }
        }
        Ok(1)
    }

    fn list_tests(
        &self,
        module: &Module,
        arguments: &[String],
        root: &Path,
    ) -> Result<Vec<String>, String> {
        let mut listing: Vec<String> = arguments
            .iter()
            .filter(|argument| {
                !argument.starts_with("--test-threads") && argument.as_str() != "--nocapture"
            })
            .cloned()
            .collect();
        listing.extend(["--list", "--format", "terse"].map(str::to_owned));
        let output = MemoryOutputPipe::new(LISTING_CAPACITY);
        let code = self.run_instance(module, &listing, root, Some(output.clone()))?;
        if code != 0 {
            return Err(format!("the tests could not be listed: exit {code}"));
        }
        Ok(String::from_utf8_lossy(&output.contents())
            .lines()
            .filter_map(|line| line.strip_suffix(": test"))
            .map(str::to_owned)
            .collect())
    }

    fn run_instance(
        &self,
        module: &Module,
        arguments: &[String],
        root: &Path,
        stdout: Option<MemoryOutputPipe>,
    ) -> Result<i32, String> {
        let memory = shared_memory(&self.engine, module)?;
        let directory = root
            .to_str()
            .ok_or_else(|| format!("{} is not a usable path", root.display()))?;
        let mut builder = WasiCtxBuilder::new();
        builder
            .inherit_stdio()
            .inherit_env()
            .args(arguments)
            .preopened_dir(root, directory, FsPerms::ReadWrite)
            .map_err(|error| {
                format!(
                    "{} could not be opened for the tests: {error}",
                    root.display()
                )
            })?;
        if let Some(stdout) = stdout {
            builder.stdout(stdout);
        }
        let woken: Arc<wake::Wake> = Arc::default();
        let state = State {
            wasi: builder.build_p1(),
            memory: memory.clone(),
            device: self.devices.open(),
            error: None,
            inbox: Default::default(),
            outbox: Vec::new(),
            started: Instant::now(),
            threads: threads::Spawner::new(
                self.engine.clone(),
                module.clone(),
                memory.clone(),
                Arc::clone(&woken),
            ),
            wake: Arc::clone(&woken),
        };
        let mut store = Store::new(&self.engine, state);
        let mut linker: Linker<State> = Linker::new(&self.engine);
        p1::add_to_linker_sync(&mut linker, |state: &mut State| &mut state.wasi)
            .map_err(|error| format!("wasi could not be linked: {error}"))?;
        linker
            .define(&store, "env", "memory", memory)
            .map_err(|error| format!("the test memory could not be linked: {error}"))?;
        gpu::link(&mut linker)?;
        transport::link(&mut linker)?;
        threads::link(&mut linker)?;
        wake::link(&mut linker)?;
        linker
            .define_unknown_imports_as_traps(module)
            .map_err(|error| format!("the test imports could not be stubbed: {error}"))?;
        let instance = linker
            .instantiate(&mut store, module)
            .map_err(|error| format!("the tests could not be instantiated: {error}"))?;
        let start: TypedFunc<(), ()> = instance
            .get_typed_func(&mut store, START)
            .map_err(|error| format!("the tests have no usable {START} export: {error}"))?;
        let outcome = start.call(&mut store, ());
        let state = store.data_mut();
        if let Some(failure) = state.take_error().or_else(|| state.threads.take_failure()) {
            return Err(failure);
        }
        match outcome {
            Ok(()) => Ok(0),
            Err(error) => match error.downcast_ref::<I32Exit>() {
                Some(exit) => Ok(exit.0),
                None => Err(format!("the tests trapped: {error:?}")),
            },
        }
    }
}
