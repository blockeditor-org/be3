//! The code cache: what V8 optimised of a module, kept on the device so a later start loads it
//! instead of compiling it again.
//!
//! V8 serializes only optimised (TurboFan) code, and takes back only what was serialized under
//! the same V8 and the same flags, so an entry is what a session tiered up, written as it runs
//! (every `PERSIST_EVERY`, and on `persist`), and found under the V8 version, a hash of the
//! process's flags and the module's SHA-256. A missing, stale or corrupt entry costs nothing but
//! the compile it would have saved: V8 refuses it and compiles the module. Entries are written to
//! a temporary file and renamed, so a reader never sees half of one.
//!
//! Nothing is precompiled: the cache is filled on the device by running, never by the build or
//! shipped (the host tests are handed one their build made by running a host; crates/be-host-tests).

use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock},
    time::Duration,
};

use crate::engine::{Compiled, Module};

const PERSIST_EVERY: Duration = Duration::from_secs(20);

struct Cache {
    directory: PathBuf,
    writes: bool,
    /// Each module compiled since the process started, with its entry's path and the size of
    /// what was last written there.
    modules: Mutex<Vec<(Module, PathBuf, usize)>>,
}

static CACHE: OnceLock<Cache> = OnceLock::new();

/// Keeps optimised code under `directory`, a directory of the device's own, apart from any data,
/// or with `writes` false only reads what is there (a test's, made by its build). Set once,
/// before any module is compiled.
pub fn cache_code_in(directory: PathBuf, writes: bool) {
    let _ = CACHE.set(Cache {
        directory,
        writes,
        modules: Mutex::new(Vec::new()),
    });
}

/// Starts the thread that persists every `PERSIST_EVERY`, from a thread that has compiled: on a
/// CPU with memory protection keys V8 guards its code with a key it allocates as it starts, and a
/// thread only has the rights of the thread that made it, so one made before V8 started faults on
/// the first read of its code.
fn persist_periodically() {
    static STARTED: std::sync::Once = std::sync::Once::new();
    STARTED.call_once(|| {
        std::thread::Builder::new()
            .name("wasm code cache".to_owned())
            .spawn(|| {
                loop {
                    std::thread::sleep(PERSIST_EVERY);
                    persist();
                }
            })
            .ok();
    });
}

/// Writes what V8 has optimised since the last write, for every module this process compiled.
pub fn persist() {
    let Some(cache) = CACHE.get().filter(|cache| cache.writes) else {
        return;
    };
    let mut modules = cache.modules.lock().unwrap_or_else(|held| held.into_inner());
    for (module, path, written) in modules.iter_mut() {
        let Some(mut bytes) = serialized(&module.module) else {
            continue;
        };
        fresh_budgets(&mut bytes, module.shape.functions);
        if bytes.len() <= *written {
            continue;
        }
        if write(path, &bytes).is_ok() {
            *written = bytes.len();
        }
    }
}

/// V8's default tiering budget (`--wasm-tiering-budget`).
const TIERING_BUDGET: u32 = 13_000_000;

/// Gives every function in a serialized module a fresh tiering budget. V8 ends the entry with what
/// is left of each defined function's budget, as a little-endian `u32`, and takes it back as it
/// loads; left as the session spent it, every function hot then is optimised early in each later
/// start, needed or not.
fn fresh_budgets(bytes: &mut [u8], functions: usize) {
    let Some(start) = bytes.len().checked_sub(functions * 4) else {
        return;
    };
    for budget in bytes[start..].chunks_exact_mut(4) {
        budget.copy_from_slice(&TIERING_BUDGET.to_le_bytes());
    }
}

fn write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension(format!("{}.part", std::process::id()));
    std::fs::write(&temporary, bytes)?;
    std::fs::rename(&temporary, path)
}

/// Where the entry for a module with these bytes lives, when there is a cache.
pub(crate) fn entry(bytes: &[u8]) -> Option<PathBuf> {
    let cache = CACHE.get()?;
    let version = format!("{}-{}", v8::V8::get_version(), crate::engine::flags_hash());
    Some(
        cache
            .directory
            .join(version)
            .join(format!("{}.v8", be_wasm_dylink::identity(bytes))),
    )
}

/// The module already compiled from bytes whose entry is `path`, if any. Each compile of the same
/// bytes must be handed that one: V8 shares one compiled module between compiles of the same
/// bytes only when it compiles them itself, so a second module deserialized from the entry would
/// be optimised again, apart, function by function.
pub(crate) fn compiled(path: &Path) -> Option<Module> {
    let cache = CACHE.get()?;
    let modules = cache.modules.lock().unwrap_or_else(|held| held.into_inner());
    modules
        .iter()
        .find(|(_, entry, _)| entry == path)
        .map(|(module, _, _)| Arc::clone(module))
}

/// Remembers a module compiled from bytes whose entry is `path`, to persist what it optimises.
pub(crate) fn remember(module: &Arc<Compiled>, path: PathBuf, found: usize) {
    if let Some(cache) = CACHE.get() {
        if cache.writes {
            persist_periodically();
        }
        cache
            .modules
            .lock()
            .unwrap_or_else(|held| held.into_inner())
            .push((Arc::clone(module), path, found));
    }
}

thread_local! {
    /// The entry the next caching callback on this thread hands V8.
    static CACHED: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

// unsafe(ffi)
unsafe extern "C" fn hand_cached(interface: *mut v8::ModuleCachingInterface) {
    // V8 hands the callback the compilation's caching interface for the length of the call.
    // unsafe(ffi)
    let interface = unsafe { &mut *interface };
    CACHED.with(|cached| {
        interface.set_cached_compiled_module_bytes(&cached.borrow());
    });
}

/// Compiles `bytes` with `cached`, a serialized entry V8 takes in place of compiling where it
/// matches and compiles the bytes where it does not, waiting for V8 to finish.
pub(crate) fn compile_with<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    bytes: &[u8],
    cached: Vec<u8>,
) -> Option<v8::Local<'s, v8::WasmModuleObject>> {
    CACHED.with(|slot| *slot.borrow_mut() = cached);
    let mut compilation = v8::WasmModuleCompilation::new();
    compilation.set_has_compiled_module_bytes();
    compilation.on_bytes_received(bytes);
    let done: std::rc::Rc<RefCell<Option<Option<v8::Global<v8::WasmModuleObject>>>>> =
        std::rc::Rc::default();
    let answer = std::rc::Rc::clone(&done);
    compilation.finish(scope, Some(hand_cached), move |isolate, result| {
        *answer.borrow_mut() = Some(result.ok().map(|module| v8::Global::new(isolate, module)));
    });
    let platform = crate::engine::platform();
    while done.borrow().is_none() {
        v8::Platform::pump_message_loop(&platform, scope, true);
    }
    CACHED.with(|slot| slot.borrow_mut().clear());
    let module = done.borrow_mut().take().flatten()?;
    Some(v8::Local::new(scope, module))
}

/// `v8::CompiledWasmModule::Serialize`, which rusty_v8 does not bind, called by its symbol in the
/// prebuilt library (third-party/v8). `OwnedBuffer` holds a `std::unique_ptr<const uint8_t[]>`,
/// which Chromium's libc++ makes trivial for calls, so under the Itanium ABI (Linux, macOS,
/// Android) the method takes `this` as its argument and returns the buffer and its size in
/// registers, as a function returning a two-word struct does. The buffer is freed with the
/// prebuilt's `operator delete[]`. Windows' ABI returns it through a pointer instead; it is not
/// checked there yet, so Windows keeps no cache.
#[repr(C)]
struct OwnedBuffer {
    buffer: *mut u8,
    size: usize,
}

#[cfg(not(windows))]
// unsafe(ffi)
unsafe extern "C" {
    #[link_name = "_ZN2v818CompiledWasmModule9SerializeEv"]
    fn compiled_wasm_module_serialize(this: *mut std::ffi::c_void) -> OwnedBuffer;
    #[link_name = "_ZdaPv"]
    fn operator_delete_array(pointer: *mut u8);
}

#[cfg(not(windows))]
pub(crate) fn serialized(module: &v8::CompiledWasmModule) -> Option<Vec<u8>> {
    // A v8::CompiledWasmModule is the pointer to its C++ object (wasm.rs in the v8 crate).
    // unsafe(memory)
    let this = unsafe { *std::ptr::from_ref(module).cast::<*mut std::ffi::c_void>() };
    // The prebuilt's own method on its own object; see OwnedBuffer.
    // unsafe(ffi)
    let owned = unsafe { compiled_wasm_module_serialize(this) };
    if owned.buffer.is_null() {
        return None;
    }
    // V8 gave a buffer of `size` bytes, freed only below.
    // unsafe(ffi)
    let bytes = unsafe { std::slice::from_raw_parts(owned.buffer, owned.size) }.to_vec();
    // unsafe(ffi)
    unsafe { operator_delete_array(owned.buffer) };
    (!bytes.is_empty()).then_some(bytes)
}

#[cfg(windows)]
pub(crate) fn serialized(_: &v8::CompiledWasmModule) -> Option<Vec<u8>> {
    None
}
