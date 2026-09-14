use std::{path::Path, time::Instant};

use block_gpu_host::Gpu;
use wasmtime::{Linker, Store, TypedFunc};
use wasmtime_wasi::{FsPerms, I32Exit, WasiCtxBuilder, p1};

use crate::{Host, gpu, precompile::test_module, shared_memory, state::State, threads, transport};

const START: &str = "_start";

impl Host {
    pub fn run_tests(&self, wasm: &Path, arguments: &[String], root: &Path) -> Result<i32, String> {
        let module = test_module(&self.engine, wasm)?;
        let memory = shared_memory(&self.engine, &module)?;
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
        let state = State {
            wasi: builder.build_p1(),
            memory: memory.clone(),
            gpu: Gpu::new(self.device.clone(), self.queue.clone()),
            inbox: Default::default(),
            outbox: Vec::new(),
            started: Instant::now(),
            threads: threads::Spawner::new(self.engine.clone(), module.clone(), memory.clone()),
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
        linker
            .define_unknown_imports_as_traps(&module)
            .map_err(|error| format!("the test imports could not be stubbed: {error}"))?;
        let instance = linker
            .instantiate(&mut store, &module)
            .map_err(|error| format!("the tests could not be instantiated: {error}"))?;
        let start: TypedFunc<(), ()> = instance
            .get_typed_func(&mut store, START)
            .map_err(|error| format!("the tests have no usable {START} export: {error}"))?;
        let outcome = start.call(&mut store, ());
        let state = store.data_mut();
        if let Some(failure) = state
            .gpu
            .take_error()
            .or_else(|| state.threads.take_failure())
        {
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
