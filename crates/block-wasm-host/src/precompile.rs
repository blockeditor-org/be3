use std::{
    num::NonZeroUsize,
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
    time::SystemTime,
};

use wasmtime::{Engine, Module, OptLevel};

use crate::{Flavor, PRECOMPILED_EXTENSION, Stored, configuration, module};

struct Engines {
    plugin: Engine,
    pure: Engine,
}

impl Engines {
    fn new(target: Option<&str>, quick: bool) -> Result<Self, String> {
        Ok(Self {
            plugin: compiler(Flavor::Plugin, target, quick)?,
            pure: compiler(Flavor::Pure, target, quick)?,
        })
    }

    fn for_module(&self, bytes: &[u8]) -> &Engine {
        match Module::validate(&self.pure, bytes) {
            Ok(()) => &self.pure,
            Err(_) => &self.plugin,
        }
    }
}

fn compiler(flavor: Flavor, target: Option<&str>, quick: bool) -> Result<Engine, String> {
    let mut configuration = configuration(flavor);
    if quick {
        configuration.cranelift_opt_level(OptLevel::None);
    }
    if let Some(target) = target {
        configuration
            .target(target)
            .map_err(|error| format!("{target} is not a target wasmtime compiles for: {error}"))?;
    }
    Engine::new(&configuration).map_err(|error| format!("the wasm engine could not start: {error}"))
}

pub fn precompile(modules: &[PathBuf], target: Option<&str>) -> Result<(), String> {
    let engines = &Engines::new(target, false)?;
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
                        if let Err(error) = precompile_file(engines, wasm) {
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

pub fn precompile_to(
    wasm: &Path,
    artifact: &Path,
    target: Option<&str>,
    quick: bool,
) -> Result<(), String> {
    write_precompiled(&Engines::new(target, quick)?, wasm, artifact)
}

fn precompile_file(engines: &Engines, wasm: &Path) -> Result<(), String> {
    write_precompiled(engines, wasm, &wasm.with_extension(PRECOMPILED_EXTENSION))
}

fn write_precompiled(engines: &Engines, wasm: &Path, artifact: &Path) -> Result<(), String> {
    let bytes = std::fs::read(wasm)
        .map_err(|error| format!("{} could not be read: {error}", wasm.display()))?;
    let compiled = engines
        .for_module(&bytes)
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
        return Stored::File(artifact.to_path_buf())
            .deserialize(engine)
            .map_err(|error| format!("{} could not be used: {error}", artifact.display()));
    }
    if modified(&wasm.with_extension(PRECOMPILED_EXTENSION)) > modified(wasm)
        && let Some(compiled) = wasm.to_str().and_then(|wasm| {
            crate::precompiled(engine, wasm, &|name| Stored::file(PathBuf::from(name)))
        })
    {
        return Ok(compiled);
    }
    module(engine, Stored::file(wasm.to_path_buf())?)
}

fn modified(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .ok()
}
