//! The code cache: what V8 optimised of a module, kept on the device so a later start loads it
//! instead of compiling it again.
//!
//! V8 serializes only optimised (TurboFan) code, and takes back only what was serialized under
//! the same V8 and the same flags, so an entry is what a session tiered up, written as it runs
//! (every `PERSIST_EVERY`, and on `persist`), and found under the V8 version, a hash of the
//! process's flags and the module's SHA-256. A missing, stale or corrupt entry costs nothing but
//! the compile it would have saved: `read` refuses one that is not whole or not the module's (V8
//! trusts what it is handed past its header), and V8 one from another V8, flags or CPU. Entries
//! are written to a temporary file, synced and renamed, so a reader never sees half of one.
//!
//! Nothing is precompiled: the cache is filled on the device by running, never by the build or
//! shipped (the host tests are handed one their build made by running a host; crates/be-host-tests).

use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    sync::{Arc, Condvar, Mutex, OnceLock},
    time::Duration,
};

use crate::engine::{Compiled, Module};

const PERSIST_EVERY: Duration = Duration::from_secs(20);

/// The longest an exiting host waits for the cache's thread to write.
const PERSIST_AT_EXIT: Duration = Duration::from_secs(5);

struct Cache {
    directory: PathBuf,
    writes: bool,
    /// Each module compiled since the process started that something still holds, with its
    /// entry's path and the size of what was last written there.
    modules: Mutex<Vec<(Module, PathBuf, usize)>>,
    /// Held across a compile through the cache, so two threads compiling the same bytes share one
    /// module rather than each deserializing its own.
    compiling: Mutex<()>,
    /// Writes asked of the cache's thread, and writes it finished.
    asked: Mutex<(u64, u64)>,
    changed: Condvar,
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
        compiling: Mutex::new(()),
        asked: Mutex::new((0, 0)),
        changed: Condvar::new(),
    });
}

/// Starts the cache's thread, which writes every `PERSIST_EVERY` and when `persist` asks, from a
/// thread that has compiled: on a CPU with memory protection keys V8 guards its code with a key it
/// allocates as it starts, and a thread only has the rights of the thread that made it, so one
/// made before V8 started (perhaps the one exiting) faults on the first read of its code.
fn start_writing(cache: &'static Cache) {
    static STARTED: std::sync::Once = std::sync::Once::new();
    STARTED.call_once(|| {
        prune(&cache.directory, &cache.directory.join(version()));
        std::thread::Builder::new()
            .name("wasm code cache".to_owned())
            .spawn(move || {
                let mut done = 0;
                loop {
                    let asked = {
                        let asked = lock(&cache.asked);
                        let (asked, _) = cache
                            .changed
                            .wait_timeout_while(asked, PERSIST_EVERY, |(asked, _)| *asked == done)
                            .unwrap_or_else(|held| held.into_inner());
                        asked.0
                    };
                    write_all(cache);
                    done = asked;
                    lock(&cache.asked).1 = done;
                    cache.changed.notify_all();
                }
            })
            .ok();
    });
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|held| held.into_inner())
}

/// Writes what V8 has optimised since the last write, for every module this process compiled,
/// on the cache's thread, waiting at most `PERSIST_AT_EXIT` for it.
pub fn persist() {
    let Some(cache) = CACHE.get().filter(|cache| cache.writes) else {
        return;
    };
    let mut asked = lock(&cache.asked);
    asked.0 += 1;
    let wanted = asked.0;
    cache.changed.notify_all();
    let _ = cache
        .changed
        .wait_timeout_while(asked, PERSIST_AT_EXIT, |(_, done)| *done < wanted);
}

fn write_all(cache: &Cache) {
    // Serialized apart from the lock, which every compile through the cache takes.
    let modules: Vec<(Module, PathBuf, usize)> = lock(&cache.modules).clone();
    for (module, path, written) in modules {
        let Some(mut bytes) = serialized(&module.module) else {
            continue;
        };
        fresh_budgets(&mut bytes, module.shape.functions);
        if bytes.len() <= written || write(&path, &bytes).is_err() {
            continue;
        }
        if let Some(entry) = lock(&cache.modules)
            .iter_mut()
            .find(|(held, _, _)| Arc::ptr_eq(held, &module))
        {
            entry.2 = bytes.len();
        }
    }
    // A module only the cache holds is written for the last time: a later compile of its bytes
    // reads the entry.
    lock(&cache.modules).retain(|(module, _, _)| Arc::strong_count(module) > 1);
}

/// V8's default tiering budget (`--wasm-tiering-budget`).
const TIERING_BUDGET: u32 = 13_000_000;

/// Gives every function in a serialized module a fresh tiering budget. V8 ends the entry with what
/// is left of each defined function's budget, as a little-endian `u32`, and takes it back as it
/// loads; left as the session spent it, every function hot then is optimised early in each later
/// start, needed or not. The layout is V8 152.2.0's (`WriteTieringBudget`, last in
/// `NativeModuleSerializer::Write`, wasm-serialization.cc); check it again with each new V8.
fn fresh_budgets(bytes: &mut [u8], functions: usize) {
    let Some(start) = bytes.len().checked_sub(functions * 4) else {
        return;
    };
    for budget in bytes[start..].chunks_exact_mut(4) {
        budget.copy_from_slice(&TIERING_BUDGET.to_le_bytes());
    }
}

/// What starts an entry: then the module's SHA-256 and the payload's, in hex, then the payload V8
/// serialized. V8 checks only its own header and reads the rest unchecked (it trusts the
/// embedder), so a truncated, corrupt or misnamed entry is refused here, before V8 sees it. The
/// cache trusts whoever can write to its directory, as a browser's code cache does: the
/// directory is the user's own (made 0700), never one a guest names.
const MAGIC: &[u8; 8] = b"be3-v8c\0";
const HEADER: usize = MAGIC.len() + 64 + 64;

/// The module identity an entry's path names: its file stem.
fn named(path: &Path) -> Option<&str> {
    path.file_stem()?.to_str()
}

pub(crate) fn write(path: &Path, payload: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let module = named(path).ok_or(std::io::ErrorKind::InvalidInput)?;
    if let Some(parent) = path.parent() {
        create_private(parent)?;
    }
    let temporary = path.with_extension(format!("{}.part", std::process::id()));
    let written = (|| {
        let mut file = std::fs::File::create(&temporary)?;
        file.write_all(MAGIC)?;
        file.write_all(module.as_bytes())?;
        file.write_all(be_wasm_dylink::identity(payload).as_bytes())?;
        file.write_all(payload)?;
        // Durable before it is named, so a crash leaves the old entry or the whole new one.
        file.sync_all()
    })()
    .and_then(|()| std::fs::rename(&temporary, path));
    if written.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    written
}

/// Creates `directory` and those above it, each only its owner may enter.
fn create_private(directory: &Path) -> std::io::Result<()> {
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder.create(directory)
}

/// The payload of the entry at `path`, if it is whole and is the one for the module it names.
pub(crate) fn read(path: &Path) -> Option<Vec<u8>> {
    let module = named(path)?;
    let entry = std::fs::read(path).ok()?;
    let (header, payload) = entry.split_at_checked(HEADER)?;
    let (magic, hashes) = header.split_at(MAGIC.len());
    let (named_module, payload_hash) = hashes.split_at(64);
    (magic == MAGIC
        && named_module == module.as_bytes()
        && payload_hash == be_wasm_dylink::identity(payload).as_bytes())
    .then(|| payload.to_vec())
}

/// Removes what earlier V8 versions or flags left beside `current`, which no start reads again.
fn prune(directory: &Path, current: &Path) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path != current && entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            let _ = std::fs::remove_dir_all(path);
        }
    }
}

/// The directory of entries this V8 with these flags reads.
fn version() -> String {
    format!("{}-{}", v8::V8::get_version(), crate::engine::flags_hash())
}

/// Where the entry for a module with these bytes lives, when there is a cache.
pub(crate) fn entry(bytes: &[u8]) -> Option<PathBuf> {
    let cache = CACHE.get()?;
    Some(
        cache
            .directory
            .join(version())
            .join(format!("{}.v8", be_wasm_dylink::identity(bytes))),
    )
}

/// Holds every other compile through the cache off until the returned guard drops.
pub(crate) fn compiling() -> Option<std::sync::MutexGuard<'static, ()>> {
    Some(lock(&CACHE.get()?.compiling))
}

/// The module already compiled from bytes whose entry is `path`, if any. Each compile of the same
/// bytes must be handed that one: V8 shares one compiled module between compiles of the same
/// bytes only when it compiles them itself, so a second module deserialized from the entry would
/// be optimised again, apart, function by function.
pub(crate) fn compiled(path: &Path) -> Option<Module> {
    lock(&CACHE.get()?.modules)
        .iter()
        .find(|(_, entry, _)| entry == path)
        .map(|(module, _, _)| Arc::clone(module))
}

/// Remembers a module compiled from bytes whose entry is `path`, to persist what it optimises.
/// `found` is the size of the entry V8 took, which a write must outgrow, or 0 when it took none.
pub(crate) fn remember(module: &Arc<Compiled>, path: PathBuf, found: usize) {
    if let Some(cache) = CACHE.get() {
        if cache.writes {
            start_writing(cache);
        }
        lock(&cache.modules).push((Arc::clone(module), path, found));
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
) -> Result<v8::Local<'s, v8::WasmModuleObject>, String> {
    type Answer = Result<v8::Global<v8::WasmModuleObject>, v8::Global<v8::Value>>;
    CACHED.with(|slot| *slot.borrow_mut() = cached);
    let mut compilation = v8::WasmModuleCompilation::new();
    compilation.set_has_compiled_module_bytes();
    compilation.on_bytes_received(bytes);
    let done: std::rc::Rc<RefCell<Option<Answer>>> = std::rc::Rc::default();
    let answer = std::rc::Rc::clone(&done);
    compilation.finish(scope, Some(hand_cached), move |isolate, result| {
        *answer.borrow_mut() = Some(match result {
            Ok(module) => Ok(v8::Global::new(isolate, module)),
            Err(error) => Err(v8::Global::new(isolate, error)),
        });
    });
    let platform = crate::engine::platform();
    while done.borrow().is_none() {
        v8::Platform::pump_message_loop(&platform, scope, true);
    }
    CACHED.with(|slot| slot.borrow_mut().clear());
    let answer = done.borrow_mut().take();
    match answer {
        Some(Ok(module)) => Ok(v8::Local::new(scope, module)),
        Some(Err(error)) => Err(v8::Local::new(scope, error).to_rust_string_lossy(scope)),
        None => Err("V8 gave no answer".to_owned()),
    }
}

/// `v8::CompiledWasmModule::Serialize`, which rusty_v8 does not bind, called by its symbol in the
/// prebuilt library (third-party/v8). `OwnedBuffer` holds a `std::unique_ptr<const uint8_t[]>`,
/// which Chromium's libc++ makes trivial for calls, so under the Itanium ABI (Linux, macOS,
/// Android) the method takes `this` as its argument and returns the buffer and its size in
/// registers, as a function returning a two-word struct does. The buffer is freed with the
/// prebuilt's `operator delete[]`. Windows' ABI returns it through a pointer instead; it is not
/// checked there yet, so Windows keeps no cache.
#[repr(C)]
#[cfg(not(windows))]
struct OwnedBuffer {
    buffer: *mut u8,
    size: usize,
}

// unsafe(ffi)
#[cfg(not(windows))]
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
