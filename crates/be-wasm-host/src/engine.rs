//! V8, which every guest runs on: the process's one platform and its flags, a module as it is
//! compiled once and shared between isolates, what a module imports and exports (read from its
//! bytes, as V8 says only part of it), the memory cap of a pure guest, and the few JavaScript API
//! calls (`WebAssembly.Memory`, `Instance`, `Global`, a table's `grow` and `set`) the host makes
//! from Rust. No JavaScript is compiled: the host reaches the API through V8's object calls.

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use wasmparser::{ExternalKind, Parser, Payload, TypeRef, Validator, WasmFeatures};

/// V8 keeps this many frames of a trap's stack, which is the backtrace an app's failure shows.
const BACKTRACE_FRAMES: usize = 100;

pub(crate) const PAGE: u64 = 65536;

/// The most entries a table may hold: 8 MB of V8's heap. Rust's function tables hold the
/// functions whose addresses are taken, far fewer.
const TABLE_ENTRIES: u32 = 1 << 20;

static PLATFORM: OnceLock<v8::SharedRef<v8::Platform>> = OnceLock::new();
static FLAGS: OnceLock<String> = OnceLock::new();

/// The platform V8 runs on, whose message loop a compilation that finishes later is pumped on.
pub(crate) fn platform() -> v8::SharedRef<v8::Platform> {
    initialize();
    PLATFORM
        .get()
        .cloned()
        .unwrap_or_else(|| unreachable!("V8 is initialized"))
}

/// A hash of the flags V8 runs with, which code cached under other flags is kept apart by.
pub(crate) fn flags_hash() -> String {
    initialize();
    be_wasm_dylink::identity(FLAGS.get().map_or("", String::as_str).as_bytes())[..16].to_owned()
}

/// Starts V8 once for the process. `--wasm-unsafe-fast-api-wrapper` lets a wasm import call a
/// fast-API function directly, which is how every gpu and transport call reaches the host without
/// JavaScript in between (gpu.rs). With `BE3_PERF_MAP` set, V8 writes the perf map that names the
/// guests' compiled functions for `perf` (guides/performance.md).
pub(crate) fn initialize() {
    static STARTED: OnceLock<()> = OnceLock::new();
    STARTED.get_or_init(|| {
        // The fast-API wrapper skips the checks V8 makes when a wasm import is an ordinary
        // function: that its arguments convert to what the function's signature names. Every
        // fast function here takes exactly the import's own wasm types (gpu.rs generates its
        // signature and its CFunctionInfo from one list), so there is nothing a guest's call
        // could convert wrongly; an import with another signature fails to link instead.
        //
        // A table holds at most TABLE_ENTRIES, so the one table a guest may have (Shape::bounded)
        // takes a bounded share of its isolate's heap.
        let mut flags = format!(
            "--wasm-unsafe-fast-api-wrapper --stack-trace-limit={BACKTRACE_FRAMES} \
             --wasm-max-table-size={TABLE_ENTRIES}"
        );
        if std::env::var_os("BE3_PERF_MAP").is_some() {
            flags.push_str(" --perf-basic-prof");
        }
        v8::V8::set_flags_from_string(&flags);
        let _ = FLAGS.set(flags);
        let platform = v8::new_default_platform(0, false).make_shared();
        let _ = PLATFORM.set(platform.clone());
        v8::V8::initialize_platform(platform);
        v8::V8::initialize();
    });
}

/// The most a guest's isolate holds on V8's own heap (its table, exceptions and the host's few
/// objects; a guest's linear memory is apart from it), and how far past it the isolate may go
/// while it stops. Only a backstop: V8 does not always deliver the termination once a heap is
/// held at its limit (with its tiering, measured on 152.2.0), so Shape::bounded keeps every guest
/// far under it.
const HEAP_LIMIT: usize = 512 << 20;
const HEAP_GRACE: usize = 64 << 20;

pub(crate) const OUT_OF_MEMORY: &str = "the app ran out of memory";

/// Whether an isolate reached HEAP_LIMIT, which V8 would otherwise answer by ending the process.
pub(crate) struct Exhausted {
    reached: std::sync::atomic::AtomicBool,
    isolate: v8::IsolateHandle,
}

impl Exhausted {
    pub(crate) fn reached(&self) -> bool {
        self.reached.load(std::sync::atomic::Ordering::SeqCst)
    }
}

// unsafe(ffi)
/// Near HEAP_LIMIT: the guest is stopped, and its isolate given the grace to unwind.
unsafe extern "C" fn near_heap_limit(
    data: *mut std::ffi::c_void,
    current: usize,
    _initial: usize,
) -> usize {
    // data is the Exhausted the isolate's owner keeps until the isolate is gone.
    // unsafe(memory)
    let exhausted = unsafe { &*data.cast::<Exhausted>() };
    // The grace is given once: should a guest fill the heap all the same, V8 then ends the
    // process near the limit rather than letting one isolate take the machine.
    if exhausted
        .reached
        .swap(true, std::sync::atomic::Ordering::SeqCst)
    {
        return current;
    }
    exhausted.isolate.terminate_execution();
    current + HEAP_GRACE
}

/// A new isolate, which one thread at a time may lock, with the one context every guest call on
/// it runs in, and its heap's watch, which must outlive it.
pub(crate) struct Isolate {
    pub(crate) shared: v8::SharedIsolate,
    pub(crate) context: v8::Global<v8::Context>,
    pub(crate) exhausted: Arc<Exhausted>,
}

pub(crate) fn isolate() -> Isolate {
    initialize();
    let params = v8::CreateParams::default().heap_limits(0, HEAP_LIMIT);
    // the isolate is new, so nothing holds a weak handle to it or a cppgc heap.
    // unsafe(ffi)
    let shared = unsafe { v8::Isolate::new(params).try_into_shared() }
        .unwrap_or_else(|_| panic!("a new isolate could not be shared"));
    let exhausted = Arc::new(Exhausted {
        reached: std::sync::atomic::AtomicBool::new(false),
        isolate: shared.thread_safe_handle(),
    });
    let context = {
        let mut locker = shared.lock();
        locker.add_near_heap_limit_callback(
            near_heap_limit,
            Arc::as_ptr(&exhausted).cast_mut().cast(),
        );
        v8::scope!(let scope, &mut *locker);
        let context = v8::Context::new(scope, Default::default());
        v8::Global::new(scope, context)
    };
    Isolate {
        shared,
        context,
        exhausted,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Function,
    Table,
    Memory(Memory),
    Global,
    Tag,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Memory {
    pub(crate) initial: u64,
    pub(crate) maximum: Option<u64>,
    pub(crate) shared: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct Import {
    pub(crate) module: String,
    pub(crate) name: String,
    pub(crate) kind: Kind,
}

#[derive(Clone, Debug)]
pub(crate) struct Export {
    pub(crate) name: String,
    pub(crate) kind: Kind,
}

/// What a module imports and exports, in its order.
#[derive(Clone, Debug, Default)]
pub(crate) struct Shape {
    pub(crate) imports: Vec<Import>,
    pub(crate) exports: Vec<Export>,
    /// The tables it defines.
    pub(crate) tables: usize,
    /// Whether it declares garbage-collected types (structs, arrays).
    pub(crate) collected: bool,
    /// The functions it defines.
    pub(crate) functions: usize,
}

impl Shape {
    pub(crate) fn read(bytes: &[u8]) -> Result<Self, String> {
        let invalid =
            |error: wasmparser::BinaryReaderError| format!("the module is not valid: {error}");
        let mut shape = Self::default();
        let mut memories = Vec::new();
        for payload in Parser::new(0).parse_all(bytes) {
            match payload.map_err(invalid)? {
                Payload::TypeSection(reader) => {
                    for group in reader {
                        shape.collected |= group.map_err(invalid)?.into_types().any(|kind| {
                            !matches!(
                                kind.composite_type.inner,
                                wasmparser::CompositeInnerType::Func(_)
                            )
                        });
                    }
                }
                Payload::TableSection(reader) => shape.tables += reader.count() as usize,
                Payload::FunctionSection(reader) => shape.functions = reader.count() as usize,
                Payload::ImportSection(reader) => {
                    for import in reader.into_imports() {
                        let import = import.map_err(invalid)?;
                        let kind = match import.ty {
                            TypeRef::Func(_) | TypeRef::FuncExact(_) => Kind::Function,
                            TypeRef::Table(_) => Kind::Table,
                            TypeRef::Memory(memory) => {
                                if memory.memory64 {
                                    return Err("the module imports a 64-bit memory".to_owned());
                                }
                                Kind::Memory(Memory {
                                    initial: memory.initial,
                                    maximum: memory.maximum,
                                    shared: memory.shared,
                                })
                            }
                            TypeRef::Global(_) => Kind::Global,
                            TypeRef::Tag(_) => Kind::Tag,
                        };
                        shape.imports.push(Import {
                            module: import.module.to_owned(),
                            name: import.name.to_owned(),
                            kind,
                        });
                    }
                }
                Payload::MemorySection(reader) => {
                    for memory in reader {
                        let memory = memory.map_err(invalid)?;
                        memories.push(Memory {
                            initial: memory.initial,
                            maximum: memory.maximum,
                            shared: memory.shared,
                        });
                    }
                }
                Payload::ExportSection(reader) => {
                    for export in reader {
                        let export = export.map_err(invalid)?;
                        let kind = match export.kind {
                            ExternalKind::Func | ExternalKind::FuncExact => Kind::Function,
                            ExternalKind::Table => Kind::Table,
                            ExternalKind::Memory => Kind::Memory(
                                memories
                                    .get(export.index as usize)
                                    .cloned()
                                    .unwrap_or(Memory {
                                        initial: 0,
                                        maximum: None,
                                        shared: false,
                                    }),
                            ),
                            ExternalKind::Global => Kind::Global,
                            ExternalKind::Tag => Kind::Tag,
                        };
                        shape.exports.push(Export {
                            name: export.name.to_owned(),
                            kind,
                        });
                    }
                }
                _ => {}
            }
        }
        Ok(shape)
    }

    /// Refuses what would let a guest fill V8's own heap, which V8 cannot stop it doing once the
    /// heap is at its limit (it no longer delivers a termination): garbage-collected objects,
    /// and tables past one (whose size `--wasm-max-table-size` bounds).
    pub(crate) fn bounded(&self) -> Result<(), String> {
        if self.collected {
            return Err(
                "the module declares garbage-collected types, which no guest may".to_owned(),
            );
        }
        let imported = self
            .imports
            .iter()
            .filter(|import| import.kind == Kind::Table)
            .count();
        if self.tables + imported > 1 {
            return Err("the module has more than one table, which no guest may".to_owned());
        }
        Ok(())
    }

    pub(crate) fn memory_import(&self) -> Option<&Import> {
        self.imports
            .iter()
            .find(|import| matches!(import.kind, Kind::Memory(_)))
    }
}

/// A module compiled once, which any isolate instantiates without compiling it again.
pub(crate) struct Compiled {
    pub(crate) module: v8::CompiledWasmModule,
    pub(crate) shape: Shape,
}

pub(crate) type Module = Arc<Compiled>;

pub(crate) fn compile(scope: &mut v8::PinScope<'_, '_>, bytes: &[u8]) -> Result<Module, String> {
    compile_with_cache(scope, bytes, crate::cache::entry(bytes))
}

/// Compiles `bytes` past the code cache: nothing is read from it or written to it, and the module
/// is held only by whoever holds it (the sandbox's modules from bytes).
pub(crate) fn compile_uncached(
    scope: &mut v8::PinScope<'_, '_>,
    bytes: &[u8],
) -> Result<Module, String> {
    compile_with_cache(scope, bytes, None)
}

fn compile_with_cache(
    scope: &mut v8::PinScope<'_, '_>,
    bytes: &[u8],
    entry: Option<std::path::PathBuf>,
) -> Result<Module, String> {
    let shape = Shape::read(bytes)?;
    shape.bounded()?;
    let _compiling = entry.as_ref().and_then(|_| crate::cache::compiling());
    if let Some(module) = entry.as_deref().and_then(crate::cache::compiled) {
        return Ok(module);
    }
    let cached = entry.as_deref().and_then(crate::cache::read);
    let offered = cached.as_ref().map_or(0, Vec::len);
    v8::tc_scope!(let caught, scope);
    let module = match cached {
        Some(cached) => crate::cache::compile_with(caught, bytes, cached),
        None => match v8::WasmModuleObject::compile(caught, bytes) {
            Some(module) => Ok(module),
            None => Err(thrown!(caught)),
        },
    };
    let module =
        module.map_err(|error| format!("the app module could not be compiled: {error}"))?;
    let compiled = Arc::new(Compiled {
        module: module.get_compiled_module(),
        shape,
    });
    if let Some(path) = entry {
        // What V8 took holds optimised code; an entry it refused (another CPU's) holds none of
        // this module's, and a later write replaces it whatever its size.
        let took = offered > 0 && crate::cache::serialized(&compiled.module).is_some();
        crate::cache::remember(&compiled, path, if took { offered } else { 0 });
    }
    Ok(compiled)
}

/// A pure guest's module, checked for what a pure guest may not use (threads and shared memory,
/// and relaxed SIMD, whose results differ between machines), with the memory it defines capped
/// at `limit` bytes: its maximum lowered to the cap, so the guest's growing past it fails as
/// growing past any maximum does.
pub(crate) fn pure(bytes: &[u8], limit: Option<usize>) -> Result<Vec<u8>, String> {
    // No garbage-collected objects or strings either, which live on V8's heap where the memory cap
    // does not reach, and one 32-bit memory, so the cap is the guest's whole memory.
    let features = WasmFeatures::default()
        - WasmFeatures::THREADS
        - WasmFeatures::SHARED_EVERYTHING_THREADS
        - WasmFeatures::RELAXED_SIMD
        - WasmFeatures::GC
        - WasmFeatures::MULTI_MEMORY
        - WasmFeatures::MEMORY64;
    Validator::new_with_features(features)
        .validate_all(bytes)
        .map_err(|error| format!("the guest uses what a pure guest may not: {error}"))?;
    match limit {
        Some(limit) => capped(bytes, pages(limit)),
        None => Ok(bytes.to_vec()),
    }
}

pub(crate) fn pages(limit: usize) -> u64 {
    limit as u64 / PAGE
}

/// The maximum a memory may grow to under a cap of `cap` pages.
pub(crate) fn capped_maximum(memory: &Memory, cap: u64) -> Result<u64, String> {
    if memory.initial > cap {
        return Err(format!(
            "the guest's memory starts at {} pages, past its limit of {cap}",
            memory.initial
        ));
    }
    Ok(memory.maximum.map_or(cap, |maximum| maximum.min(cap)))
}

fn capped(bytes: &[u8], cap: u64) -> Result<Vec<u8>, String> {
    let invalid =
        |error: wasmparser::BinaryReaderError| format!("the module is not valid: {error}");
    let mut module = wasm_encoder::Module::new();
    for payload in Parser::new(0).parse_all(bytes) {
        let payload = payload.map_err(invalid)?;
        if let Payload::MemorySection(reader) = &payload {
            let mut section = wasm_encoder::MemorySection::new();
            for memory in reader.clone() {
                let memory = memory.map_err(invalid)?;
                let maximum = capped_maximum(
                    &Memory {
                        initial: memory.initial,
                        maximum: memory.maximum,
                        shared: memory.shared,
                    },
                    cap,
                )?;
                section.memory(wasm_encoder::MemoryType {
                    minimum: memory.initial,
                    maximum: Some(maximum),
                    memory64: memory.memory64,
                    shared: memory.shared,
                    page_size_log2: memory.page_size_log2,
                });
            }
            module.section(&section);
            continue;
        }
        if let Some((id, range)) = payload.as_section() {
            module.section(&wasm_encoder::RawSection {
                id,
                data: &bytes[range.start as usize..range.end as usize],
            });
        }
    }
    Ok(module.finish())
}

/// A pure guest's module with the memory it defines imported instead, as `env.memory` of the same
/// type, so that it compiles once and each instance of it is given a memory capped at its own
/// limit (sandbox.rs). A module that defines no memory is unchanged.
pub(crate) fn importing_memory(bytes: &[u8]) -> Result<Vec<u8>, String> {
    use wasm_encoder::Encode;
    let invalid =
        |error: wasmparser::BinaryReaderError| format!("the module is not valid: {error}");
    let mut defined = None;
    for payload in Parser::new(0).parse_all(bytes) {
        if let Payload::MemorySection(reader) = payload.map_err(invalid)? {
            for memory in reader {
                defined = Some(memory.map_err(invalid)?);
            }
        }
    }
    let Some(memory) = defined else {
        return Ok(bytes.to_vec());
    };
    let mut import = Vec::new();
    "env".encode(&mut import);
    MEMORY_NAME.encode(&mut import);
    wasm_encoder::EntityType::Memory(wasm_encoder::MemoryType {
        minimum: memory.initial,
        maximum: memory.maximum,
        memory64: memory.memory64,
        shared: memory.shared,
        page_size_log2: memory.page_size_log2,
    })
    .encode(&mut import);
    let imports = |count: u32, entries: &[u8]| {
        let mut data = Vec::new();
        count.encode(&mut data);
        data.extend_from_slice(entries);
        data.extend_from_slice(&import);
        data
    };
    let mut module = wasm_encoder::Module::new();
    let mut placed = false;
    for payload in Parser::new(0).parse_all(bytes) {
        let payload = payload.map_err(invalid)?;
        let Some((id, range)) = payload.as_section() else {
            continue;
        };
        let section = &bytes[range.start as usize..range.end as usize];
        match &payload {
            Payload::MemorySection(_) => continue,
            Payload::ImportSection(reader) => {
                // The entries follow the count, a LEB128 whose last byte has no high bit.
                let counted = section
                    .iter()
                    .position(|byte| byte & 0x80 == 0)
                    .map_or(section.len(), |last| last + 1);
                let data = imports(reader.count() + 1, &section[counted..]);
                module.section(&wasm_encoder::RawSection { id, data: &data });
                placed = true;
                continue;
            }
            _ => {}
        }
        // A module without imports gets a section for its memory's, before the first known
        // section that follows imports.
        if !placed && id > IMPORT_SECTION {
            let data = imports(1, &[]);
            module.section(&wasm_encoder::RawSection {
                id: IMPORT_SECTION,
                data: &data,
            });
            placed = true;
        }
        module.section(&wasm_encoder::RawSection { id, data: section });
    }
    if !placed {
        let data = imports(1, &[]);
        module.section(&wasm_encoder::RawSection {
            id: IMPORT_SECTION,
            data: &data,
        });
    }
    Ok(module.finish())
}

const IMPORT_SECTION: u8 = 2;

const MEMORY_NAME: &str = "memory";

/// The host functions and values an isolate offers its guests, by module and name.
#[derive(Default)]
pub(crate) struct Defined<'s>(HashMap<(String, String), v8::Local<'s, v8::Value>>);

impl<'s> Defined<'s> {
    pub(crate) fn define(&mut self, module: &str, name: &str, value: v8::Local<'s, v8::Value>) {
        self.0.insert((module.to_owned(), name.to_owned()), value);
    }

    pub(crate) fn get(&self, module: &str, name: &str) -> Option<v8::Local<'s, v8::Value>> {
        self.0.get(&(module.to_owned(), name.to_owned())).copied()
    }
}

/// Makes `defined[module][name]` an ordinary host function with `data` (its State).
pub(crate) fn function<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    defined: &mut Defined<'s>,
    module: &str,
    name: &str,
    data: v8::Local<'s, v8::Value>,
    callback: impl v8::MapFnTo<v8::FunctionCallback>,
) -> Result<(), String> {
    let function = v8::Function::builder(callback)
        .data(data)
        .build(scope)
        .ok_or_else(|| format!("{module}.{name} could not be made"))?;
    defined.define(module, name, function.into());
    Ok(())
}

/// The names the module whose start `bytes` is imports, once `bytes` reaches past its imports.
pub(crate) fn imports_in(bytes: &[u8]) -> Option<Vec<String>> {
    for payload in Parser::new(0).parse_all(bytes) {
        match payload.ok()? {
            Payload::ImportSection(reader) => {
                return reader
                    .into_imports()
                    .map(|import| import.map(|import| import.name.to_owned()).ok())
                    .collect();
            }
            Payload::FunctionSection(_) | Payload::CodeSectionStart { .. } | Payload::End(_) => {
                return Some(Vec::new());
            }
            _ => {}
        }
    }
    None
}

/// The exports the host itself calls, which no module imports.
const HOST_ENTRIES: [&str; 6] = [
    "_initialize",
    "_start",
    "wasi_thread_start",
    "app_start",
    "app_step",
    "app_shutdown",
];

/// A library's module with only the function and global exports `kept` names, and every export
/// the loader reads itself (`__`-prefixed). A library exports every symbol, since it cannot know
/// what an app will use, and instantiating makes a function object and a property for each: in
/// beui's, a hundred thousand of them, which was most of what instantiating an app cost.
pub(crate) fn trimmed(bytes: &[u8], kept: impl Fn(&str) -> bool) -> Result<Vec<u8>, String> {
    let invalid =
        |error: wasmparser::BinaryReaderError| format!("the module is not valid: {error}");
    let mut module = wasm_encoder::Module::new();
    for payload in Parser::new(0).parse_all(bytes) {
        let payload = payload.map_err(invalid)?;
        if let Payload::ExportSection(reader) = &payload {
            let mut section = wasm_encoder::ExportSection::new();
            for export in reader.clone() {
                let export = export.map_err(invalid)?;
                let kind = match export.kind {
                    ExternalKind::Func | ExternalKind::FuncExact => wasm_encoder::ExportKind::Func,
                    ExternalKind::Table => wasm_encoder::ExportKind::Table,
                    ExternalKind::Memory => wasm_encoder::ExportKind::Memory,
                    ExternalKind::Global => wasm_encoder::ExportKind::Global,
                    ExternalKind::Tag => wasm_encoder::ExportKind::Tag,
                };
                let named = matches!(
                    kind,
                    wasm_encoder::ExportKind::Func | wasm_encoder::ExportKind::Global
                );
                if !named
                    || export.name.starts_with("__")
                    || HOST_ENTRIES.contains(&export.name)
                    || kept(export.name)
                {
                    section.export(export.name, kind, export.index);
                }
            }
            module.section(&section);
            continue;
        }
        if let Some((id, range)) = payload.as_section() {
            module.section(&wasm_encoder::RawSection {
                id,
                data: &bytes[range.start as usize..range.end as usize],
            });
        }
    }
    Ok(module.finish())
}

pub(crate) fn key<'s>(scope: &mut v8::PinScope<'s, '_>, name: &str) -> v8::Local<'s, v8::Value> {
    v8::String::new(scope, name)
        .unwrap_or_else(|| v8::String::empty(scope))
        .into()
}

pub(crate) fn get<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    object: v8::Local<v8::Object>,
    name: &str,
) -> Option<v8::Local<'s, v8::Value>> {
    let name = key(scope, name);
    object
        .get(scope, name)
        .filter(|value| !value.is_undefined())
}

pub(crate) fn set(
    scope: &mut v8::PinScope<'_, '_>,
    object: v8::Local<v8::Object>,
    name: &str,
    value: v8::Local<v8::Value>,
) {
    let name = key(scope, name);
    object.set(scope, name, value);
}

/// `WebAssembly[name]`.
pub(crate) fn web_assembly<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    name: &str,
) -> Result<v8::Local<'s, v8::Function>, String> {
    let global = scope.get_current_context().global(scope);
    get(scope, global, "WebAssembly")
        .and_then(|wasm| wasm.to_object(scope))
        .and_then(|wasm| get(scope, wasm, name))
        .and_then(|constructor| constructor.try_into().ok())
        .ok_or_else(|| format!("V8 has no WebAssembly.{name}"))
}

/// Calls `object[name](...arguments)`.
pub(crate) fn call<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    object: v8::Local<v8::Object>,
    name: &str,
    arguments: &[v8::Local<v8::Value>],
) -> Option<v8::Local<'s, v8::Value>> {
    let method: v8::Local<v8::Function> = get(scope, object, name)?.try_into().ok()?;
    method.call(scope, object.into(), arguments)
}

pub(crate) fn number<'s>(scope: &mut v8::PinScope<'s, '_>, value: f64) -> v8::Local<'s, v8::Value> {
    v8::Number::new(scope, value).into()
}

/// A new `WebAssembly.Memory`.
pub(crate) fn memory<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    initial: u64,
    maximum: Option<u64>,
    shared: bool,
) -> Result<v8::Local<'s, v8::Object>, String> {
    let descriptor = v8::Object::new(scope);
    let initial = number(scope, initial as f64);
    set(scope, descriptor, "initial", initial);
    if let Some(maximum) = maximum {
        let maximum = number(scope, maximum as f64);
        set(scope, descriptor, "maximum", maximum);
    }
    if shared {
        let yes = v8::Boolean::new(scope, true).into();
        set(scope, descriptor, "shared", yes);
    }
    let constructor = web_assembly(scope, "Memory")?;
    constructor
        .new_instance(scope, &[descriptor.into()])
        .ok_or_else(|| "the app memory could not be created".to_owned())
}

/// A new `WebAssembly.Global` holding an i32.
pub(crate) fn global<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    value: i32,
    mutable: bool,
) -> Result<v8::Local<'s, v8::Value>, String> {
    let descriptor = v8::Object::new(scope);
    let i32 = key(scope, "i32");
    set(scope, descriptor, "value", i32);
    let mutable = v8::Boolean::new(scope, mutable).into();
    set(scope, descriptor, "mutable", mutable);
    let constructor = web_assembly(scope, "Global")?;
    let value = v8::Integer::new(scope, value).into();
    constructor
        .new_instance(scope, &[descriptor.into(), value])
        .map(Into::into)
        .ok_or_else(|| "an app global could not be made".to_owned())
}

/// The i32 a `WebAssembly.Global` holds.
pub(crate) fn global_value(
    scope: &mut v8::PinScope<'_, '_>,
    global: v8::Local<v8::Value>,
) -> Option<i32> {
    let global = global.to_object(scope)?;
    get(scope, global, "value")?.int32_value(scope)
}

pub(crate) fn set_global_value(
    scope: &mut v8::PinScope<'_, '_>,
    global: v8::Local<v8::Value>,
    value: i32,
) -> Option<()> {
    let global = global.to_object(scope)?;
    let value = v8::Integer::new(scope, value).into();
    let name = key(scope, "value");
    global.set(scope, name, value).map(|_| ())
}

/// What a `TryCatch` caught, with its stack where it has one, which for a trap is the guest's
/// backtrace.
macro_rules! thrown {
    ($caught:expr) => {{
        let caught = &mut *$caught;
        match caught.exception() {
            None => "nothing was thrown".to_owned(),
            Some(exception) => {
                let stack = exception
                    .to_object(caught)
                    .and_then(|object| $crate::engine::get(caught, object, "stack"))
                    .filter(|stack| stack.is_string());
                match stack {
                    Some(stack) => stack.to_rust_string_lossy(caught),
                    None => exception.to_rust_string_lossy(caught),
                }
            }
        }
    }};
}

pub(crate) use thrown;
