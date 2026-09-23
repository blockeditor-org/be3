use std::{
    num::NonZeroUsize,
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
    time::SystemTime,
};

use wasmtime::{Engine, Module};

use crate::{PRECOMPILED_EXTENSION, configuration, module, precompiled_beside};

pub fn precompile(modules: &[PathBuf], target: Option<&str>) -> Result<(), String> {
    let mut configuration = configuration();
    if let Some(target) = target {
        configuration
            .target(target)
            .map_err(|error| format!("{target} is not a target wasmtime compiles for: {error}"))?;
    }
    let engine = &Engine::new(&configuration)
        .map_err(|error| format!("the wasm engine could not start: {error}"))?;
    let next = &AtomicUsize::new(0);
    let workers = std::thread::available_parallelism()
        .map_or(1, NonZeroUsize::get)
        .min(modules.len());
    let failures: Vec<String> = std::thread::scope(|scope| {
        let workers: Vec<_> = (0..workers)
            .map(|_| {
                scope.spawn(move || {
                    let mut failures = Vec::new();
                    while let Some(wasm) = modules.get(next.fetch_add(1, Ordering::Relaxed)) {
                        if let Err(error) = precompile_file(engine, wasm) {
                            failures.push(error);
                        }
                    }
                    failures
                })
            })
            .collect();
        workers
            .into_iter()
            .flat_map(|worker| {
                worker
                    .join()
                    .unwrap_or_else(|_| vec!["a compiler thread panicked".to_owned()])
            })
            .collect()
    });
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("\n"))
    }
}

pub fn precompile_to(wasm: &Path, artifact: &Path, target: Option<&str>) -> Result<(), String> {
    let mut configuration = configuration();
    if let Some(target) = target {
        configuration
            .target(target)
            .map_err(|error| format!("{target} is not a target wasmtime compiles for: {error}"))?;
    }
    let engine = Engine::new(&configuration)
        .map_err(|error| format!("the wasm engine could not start: {error}"))?;
    write_precompiled(&engine, wasm, artifact)
}

fn precompile_file(engine: &Engine, wasm: &Path) -> Result<(), String> {
    write_precompiled(engine, wasm, &wasm.with_extension(PRECOMPILED_EXTENSION))
}

fn write_precompiled(engine: &Engine, wasm: &Path, artifact: &Path) -> Result<(), String> {
    let bytes = std::fs::read(wasm)
        .map_err(|error| format!("{} could not be read: {error}", wasm.display()))?;
    let compiled = engine
        .precompile_module(&bytes)
        .map_err(|error| format!("{} could not be compiled: {error}", wasm.display()))?;
    let partial = artifact.with_extension(format!("{PRECOMPILED_EXTENSION}.partial"));
    std::fs::write(&partial, compiled)
        .map_err(|error| format!("{} could not be written: {error}", partial.display()))?;
    std::fs::rename(&partial, artifact)
        .map_err(|error| format!("{} could not be written: {error}", artifact.display()))
}

pub(crate) fn test_module(
    engine: &Engine,
    wasm: &Path,
    precompiled: Option<&Path>,
) -> Result<Module, String> {
    if let Some(artifact) = precompiled {
        return unsafe { Module::deserialize_file(engine, artifact) }
            .map_err(|error| format!("{} could not be used: {error}", artifact.display()));
    }
    if modified(&wasm.with_extension(PRECOMPILED_EXTENSION)) > modified(wasm)
        && let Some(compiled) = precompiled_beside(engine, wasm)
    {
        return Ok(compiled);
    }
    let bytes = std::fs::read(wasm)
        .map_err(|error| format!("{} could not be read: {error}", wasm.display()))?;
    module(engine, &bytes)
}

fn modified(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .ok()
}
