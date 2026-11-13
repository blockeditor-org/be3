use std::{
    num::NonZeroUsize,
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
    time::SystemTime,
};

use wasmtime::{Engine, Module, OptLevel};

use crate::{Flavor, PRECOMPILED_EXTENSION, Stored, configuration, module};

struct Engines {
    app: Engine,
    pure: Engine,
    /// Whether to refuse a function past [`LARGEST_FUNCTION`]: in a release
    /// precompile for ARM64, whose code is what the limit is about.
    guarded: bool,
    /// Whether Cranelift panics on a function past the limit, compiling for
    /// Windows on ARM64.
    panics_on_size: bool,
}

impl Engines {
    fn new(target: Option<&str>, quick: bool) -> Result<Self, String> {
        let arm64 = target.unwrap_or(std::env::consts::ARCH).starts_with("aarch64");
        Ok(Self {
            app: compiler(Flavor::App, target, quick)?,
            pure: compiler(Flavor::Pure, target, quick)?,
            guarded: arm64 && !quick,
            panics_on_size: target.is_some_and(|target| target.starts_with("aarch64-pc-windows")),
        })
    }

    fn for_module(&self, bytes: &[u8]) -> &Engine {
        match Module::validate(&self.pure, bytes) {
            Ok(()) => &self.pure,
            Err(_) => &self.app,
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

/// The most machine code one compiled function may hold. Windows on ARM64's
/// unwind information gives a function's length in 18 bits of 4-byte words,
/// and Cranelift panics compiling for it past that, so a release precompile
/// for ARM64 refuses a module with a function past it and names it: the Linux
/// and Android ARM64 ones, which build beside Windows', name what grew.
pub const LARGEST_FUNCTION: u64 = 1 << 20;

/// The functions in a precompiled module, an ELF object with a symbol for each
/// function wasmtime compiled (its `debug_symbols`, on by default), with more
/// than `limit` bytes of code, largest first.
pub fn oversized(compiled: &[u8], limit: u64) -> Result<Vec<(String, u64)>, String> {
    let malformed = || "the precompiled module is not the ELF object wasmtime writes".to_owned();
    let bytes = |at: usize, length: usize| compiled.get(at..at + length).ok_or_else(malformed);
    let u16_at = |at| Ok::<_, String>(u16::from_le_bytes(bytes(at, 2)?.try_into().unwrap()));
    let u32_at = |at| Ok::<_, String>(u32::from_le_bytes(bytes(at, 4)?.try_into().unwrap()));
    let u64_at = |at| Ok::<_, String>(u64::from_le_bytes(bytes(at, 8)?.try_into().unwrap()));
    if bytes(0, 6)? != b"\x7fELF\x02\x01" {
        return Err(malformed());
    }
    let headers = u64_at(0x28)? as usize;
    let header_size = u16_at(0x3a)? as usize;
    let header = |index: usize| headers + index * header_size;
    let mut found = Vec::new();
    for index in 0..u16_at(0x3c)? as usize {
        const SYMBOL_TABLE: u32 = 2;
        if u32_at(header(index) + 4)? != SYMBOL_TABLE {
            continue;
        }
        let symbols = u64_at(header(index) + 24)? as usize;
        let length = u64_at(header(index) + 32)? as usize;
        let names = u64_at(header(u32_at(header(index) + 40)? as usize) + 24)? as usize;
        for symbol in (symbols..symbols + length).step_by(24) {
            let size = u64_at(symbol + 16)?;
            if size <= limit {
                continue;
            }
            let start = names + u32_at(symbol)? as usize;
            let name = compiled
                .get(start..)
                .and_then(|rest| rest.split(|byte| *byte == 0).next())
                .ok_or_else(malformed)?;
            found.push((String::from_utf8_lossy(name).into_owned(), size));
        }
    }
    found.sort_by(|a, b| b.1.cmp(&a.1));
    Ok(found)
}

fn precompile_file(engines: &Engines, wasm: &Path) -> Result<(), String> {
    write_precompiled(engines, wasm, &wasm.with_extension(PRECOMPILED_EXTENSION))
}

fn write_precompiled(engines: &Engines, wasm: &Path, artifact: &Path) -> Result<(), String> {
    let bytes = std::fs::read(wasm)
        .map_err(|error| format!("{} could not be read: {error}", wasm.display()))?;
    let engine = engines.for_module(&bytes);
    let compile = || engine.precompile_module(&bytes);
    let compiled = if engines.panics_on_size {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(compile)).map_err(|_| {
            format!(
                "{} could not be compiled: Cranelift panicked, as it does on a function of more \
                 than {LARGEST_FUNCTION} bytes of code for Windows on ARM64 (guides/app_libraries.md, \
                 Function size; the Linux ARM64 precompile names it)",
                wasm.display()
            )
        })?
    } else {
        compile()
    }
    .map_err(|error| format!("{} could not be compiled: {error}", wasm.display()))?;
    let oversized = if engines.guarded {
        oversized(&compiled, LARGEST_FUNCTION)?
    } else {
        Vec::new()
    };
    if !oversized.is_empty() {
        let listed: Vec<String> = oversized
            .iter()
            .map(|(name, size)| format!("  {size} bytes: {name}"))
            .collect();
        return Err(format!(
            "{} compiles to functions of more than {LARGEST_FUNCTION} bytes, which Windows on ARM64 \
             cannot unwind (guides/app_libraries.md, Function size):\n{}",
            wasm.display(),
            listed.join("\n")
        ));
    }
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
