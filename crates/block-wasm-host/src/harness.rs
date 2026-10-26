use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use wasmtime::{Linker, Module, Store, Trap, TypedFunc, WasmBacktrace};
use wasmtime_wasi::p2::pipe::MemoryOutputPipe;
use wasmtime_wasi::{FsPerms, I32Exit, WasiCtxBuilder, p1};

use crate::{
    Host, gpu, precompile::test_module, shared_memory, state::State, stderr::Tee, threads, wake,
};

const START: &str = "_start";
const LISTING_CAPACITY: usize = 1 << 20;
const CARRIED: [&str; 2] = ["--ignored", "--include-ignored"];
const PANICKED: &str = " panicked at ";
const ABORT: &str = "abort";
const PANIC_MACHINERY: [&str; 5] = [
    "core::panicking::",
    "core::option::expect_failed",
    "core::option::unwrap_failed",
    "core::result::unwrap_failed",
    "std::panicking::",
];
const TEST_HARNESS: &str = "test::__rust_begin_short_backtrace";
const BACKTRACE_NOTE: &str = "note: run with `RUST_BACKTRACE=1`";

impl Host {
    pub fn run_tests(
        &self,
        wasm: &Path,
        arguments: &[String],
        directories: &[PathBuf],
        precompiled: Option<&Path>,
    ) -> Result<i32, String> {
        let module = test_module(&self.engine, wasm, precompiled)?;
        match self.run_instance(&module, arguments, directories, None, &Tee::default()) {
            Ok(0) => return Ok(0),
            Ok(_) => {}
            Err(failure) => eprintln!("{failure}"),
        }
        let names = self.list_tests(&module, arguments, directories)?;
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
            let stdout = MemoryOutputPipe::new(usize::MAX);
            let stderr = Tee::held();
            let failure = match self.run_instance(
                &module,
                &single,
                directories,
                Some(stdout.clone()),
                &stderr,
            ) {
                Ok(0) => continue,
                Ok(code) => format!("exit {code}"),
                Err(failure) => failure,
            };
            print!("{}", String::from_utf8_lossy(&stdout.contents()));
            eprint!("{}", stderr.contents());
            eprintln!("{failure}");
            failed.push((name, why(&stderr, &failure)));
        }
        match failed.is_empty() {
            true => eprintln!("every test passed on its own, but not in one run together"),
            false => {
                eprintln!("{} tests failed:", failed.len());
                for (name, why) in &failed {
                    eprintln!("    {name}");
                    for line in why.lines() {
                        eprintln!("        {line}");
                    }
                }
            }
        }
        Ok(1)
    }

    fn list_tests(
        &self,
        module: &Module,
        arguments: &[String],
        directories: &[PathBuf],
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
        let code = self.run_instance(
            module,
            &listing,
            directories,
            Some(output.clone()),
            &Tee::default(),
        )?;
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
        directories: &[PathBuf],
        stdout: Option<MemoryOutputPipe>,
        stderr: &Tee,
    ) -> Result<i32, String> {
        let memory = shared_memory(&self.engine, module)?;
        let mut builder = WasiCtxBuilder::new();
        builder
            .inherit_stdio()
            .stderr(stderr.clone())
            .inherit_env()
            .args(arguments);
        for directory in directories {
            let guest = directory
                .to_str()
                .ok_or_else(|| format!("{} is not a usable path", directory.display()))?;
            builder
                .preopened_dir(directory, guest, FsPerms::ReadWrite)
                .map_err(|error| {
                    format!(
                        "{} could not be opened for the tests: {error}",
                        directory.display()
                    )
                })?;
        }
        if let Some(stdout) = stdout {
            builder.stdout(stdout);
        }
        let woken: Arc<wake::Wake> = Arc::default();
        let state = State {
            wasi: builder.build_p1(),
            memory: memory.clone(),
            device: self.devices.open(&self.imports),
            imports: Arc::clone(&self.imports),
            error: None,
            fatal: None,
            returned: Vec::new(),
            transport: Default::default(),
            gpu_time: Duration::ZERO,
            submit_time: Duration::ZERO,
            threads: threads::Spawner::new(
                self.engine.clone(),
                crate::dylink::Program::Whole(module.clone()),
                memory.clone(),
                Arc::clone(&woken),
                Arc::clone(&self.interrupt),
            ),
            wake: Arc::clone(&woken),
        };
        let mut store = Store::new(&self.engine, state);
        crate::interrupt::watch(&mut store, true);
        let mut linker: Linker<State> = Linker::new(&self.engine);
        p1::add_to_linker_sync(&mut linker, |state: &mut State| &mut state.wasi)
            .map_err(|error| format!("wasi could not be linked: {error}"))?;
        linker
            .define(&store, "env", "memory", memory)
            .map_err(|error| format!("the test memory could not be linked: {error}"))?;
        gpu::link(&mut linker)?;
        crate::panic::link(&mut linker)?;
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
                None if aborted(&error) => Err(aborted_with_backtrace(&error)),
                None => Err(format!("the tests trapped: {error:?}")),
            },
        }
    }
}

fn aborted(error: &wasmtime::Error) -> bool {
    error.downcast_ref::<Trap>() == Some(&Trap::UnreachableCodeReached)
        && error
            .downcast_ref::<WasmBacktrace>()
            .and_then(|backtrace| backtrace.frames().first())
            .and_then(|frame| frame.func_name())
            == Some(ABORT)
}

fn aborted_with_backtrace(error: &wasmtime::Error) -> String {
    let Some(backtrace) = error.downcast_ref::<WasmBacktrace>() else {
        return "the tests aborted".to_owned();
    };
    let names: Vec<String> = backtrace
        .frames()
        .iter()
        .map(|frame| match frame.func_name() {
            Some(name) => format!("{:#}", rustc_demangle::demangle(name)),
            None => format!("<function {}>", frame.func_index()),
        })
        .collect();
    let end = names
        .iter()
        .position(|name| name.starts_with(TEST_HARNESS))
        .unwrap_or(names.len());
    let start = names[..end]
        .iter()
        .rposition(|name| {
            PANIC_MACHINERY
                .iter()
                .any(|prefix| name.starts_with(prefix))
        })
        .map_or(0, |last| last + 1);
    let mut message = "the tests aborted, at:".to_owned();
    for name in &names[start..end] {
        message.push_str("\n    ");
        message.push_str(name);
    }
    message
}

fn why(stderr: &Tee, failure: &str) -> String {
    let written = stderr.contents();
    let panic = written
        .lines()
        .position(|line| line.contains(PANICKED))
        .map(|start| {
            written
                .lines()
                .skip(start)
                .filter(|line| !line.starts_with(BACKTRACE_NOTE))
                .collect::<Vec<_>>()
                .join("\n")
        });
    match panic {
        Some(panic) => format!("{panic}\n{failure}"),
        None => format!("{}\n{failure}", written.trim_end())
            .trim_start()
            .to_owned(),
    }
}
