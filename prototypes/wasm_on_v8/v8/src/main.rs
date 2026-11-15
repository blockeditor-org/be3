//! Proof of concept: run wasm on an embedded V8 (rusty_v8) with imports bound
//! to native Rust functions, no JavaScript source anywhere.
//!
//! Checks:
//! 1. instantiate a module importing a shared memory and native functions
//! 2. a native import reads guest memory (the "print" import)
//! 3. per-call cost: fast-API import vs ordinary V8 callback vs wasm->wasm
//! 4. a native import that throws stops the guest; wasm exceptions (exnref)
//! 5. TerminateExecution from another thread: a busy loop and memory.atomic.wait
//! 6. a second isolate on another thread shares the compiled module and memory
//! 7. compile and instantiate time of a big module passed on the command line
//!
//! Also `wasm-on-v8 compile A.wasm B.wasm ...` times compiling modules (V8 flags from
//! `V8_FLAGS`, e.g. `--no-wasm-lazy-compilation --no-liftoff`), and `wasm-on-v8 bench GUEST.wasm`
//! times ../guest's `run` export. ../wasmtime measures the same on wasmtime. Results are in
//! ../FINDINGS.md. Standalone cargo projects, not part of the buck build: build
//! with `cargo build --release` in each directory (the guest with
//! `--target wasm32-unknown-unknown`).

use std::ffi::c_void;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use v8::{ValueDeserializerHelper, ValueSerializerHelper};
use v8::fast_api::{self, CFunction, CFunctionInfo, Int64Representation, Type};

const GUEST: &str = r#"
(module
  (import "env" "memory" (memory 1 100 shared))
  (import "host" "add" (func $add (param i32 i32) (result i32)))
  (import "host" "add_slow" (func $add_slow (param i32 i32) (result i32)))
  (import "host" "print" (func $print (param i32 i32)))
  (import "host" "fail" (func $fail (param i32)))
  (import "host" "mix" (func $mix (param i64 f64 f32) (result f64)))
  (tag $oops (param i32))
  (data (i32.const 16) "hello from wasm, through a native import")

  (func $add_wasm (param i32 i32) (result i32) (i32.add (local.get 0) (local.get 1)))

  (func (export "hello") (call $print (i32.const 16) (i32.const 40)))

  (func (export "loop_fast") (param $n i32) (result i32) (local $acc i32)
    (block $done (loop $l
      (br_if $done (i32.eqz (local.get $n)))
      (local.set $acc (call $add (local.get $acc) (local.get $n)))
      (local.set $n (i32.sub (local.get $n) (i32.const 1)))
      (br $l)))
    (local.get $acc))

  (func (export "loop_slow") (param $n i32) (result i32) (local $acc i32)
    (block $done (loop $l
      (br_if $done (i32.eqz (local.get $n)))
      (local.set $acc (call $add_slow (local.get $acc) (local.get $n)))
      (local.set $n (i32.sub (local.get $n) (i32.const 1)))
      (br $l)))
    (local.get $acc))

  (func (export "loop_wasm") (param $n i32) (result i32) (local $acc i32)
    (block $done (loop $l
      (br_if $done (i32.eqz (local.get $n)))
      (local.set $acc (call $add_wasm (local.get $acc) (local.get $n)))
      (local.set $n (i32.sub (local.get $n) (i32.const 1)))
      (br $l)))
    (local.get $acc))

  (func (export "mix") (result f64) (call $mix (i64.const 0x100000001) (f64.const 0.5) (f32.const 0.25)))

  (func (export "call_fail") (call $fail (i32.const 7)) (unreachable))

  (func (export "catch_own") (result i32)
    (block $caught (result i32)
      (try_table (catch $oops $caught)
        (throw $oops (i32.const 42)))
      (i32.const -1)))

  (func (export "spin") (loop $l (br $l)))

  ;; blocks forever: nothing notifies address 0
  (func (export "wait") (result i32)
    (memory.atomic.wait32 (i32.const 0) (i32.const 0) (i64.const -1)))

  ;; for the second thread: store a value and notify
  (func (export "store_and_notify") (param i32)
    (i32.atomic.store (i32.const 8) (local.get 0))
    (drop (memory.atomic.notify (i32.const 8) (i32.const 1))))

  (func (export "wait_for_store") (result i32)
    (block $done (loop $l
      (br_if $done (i32.ne (i32.atomic.load (i32.const 8)) (i32.const 0)))
      (drop (memory.atomic.wait32 (i32.const 8) (i32.const 0) (i64.const -1)))
      (br $l)))
    (i32.atomic.load (i32.const 8)))
)
"#;

/// What every native import reaches through the FunctionTemplate's data.
struct HostState {
    /// Base of the shared memory. Shared memories never move: V8 reserves
    /// their maximum up front.
    memory: *const u8,
    memory_len: usize,
    printed: Mutex<Vec<String>>,
    fast_calls: AtomicU64,
    slow_calls: AtomicU64,
}

fn state(options: &fast_api::FastApiCallbackOptions) -> &'static HostState {
    let external: v8::Local<v8::External> = options.data.try_into().unwrap();
    unsafe { &*(external.value() as *const HostState) }
}

fn state_slow(args: &v8::FunctionCallbackArguments) -> &'static HostState {
    let external: v8::Local<v8::External> = args.data().try_into().unwrap();
    unsafe { &*(external.value() as *const HostState) }
}

// ---- add: fast path, with a slow fallback V8 requires but wasm never uses.

fn add_fast(
    _receiver: v8::Local<v8::Value>,
    a: i32,
    b: i32,
    options: &fast_api::FastApiCallbackOptions,
) -> i32 {
    state(options).fast_calls.fetch_add(1, Ordering::Relaxed);
    a.wrapping_add(b)
}

const ADD_FAST: CFunction = CFunction::new(
    add_fast as *const c_void,
    &CFunctionInfo::new(
        Type::Int32.as_info(),
        &[
            Type::V8Value.as_info(),
            Type::Int32.as_info(),
            Type::Int32.as_info(),
            Type::CallbackOptions.as_info(),
        ],
        Int64Representation::BigInt,
    ),
);

fn add_slow(
    scope: &mut v8::PinScope,
    args: v8::FunctionCallbackArguments,
    mut rv: v8::ReturnValue<v8::Value>,
) {
    state_slow(&args).slow_calls.fetch_add(1, Ordering::Relaxed);
    let a = args.get(0).int32_value(scope).unwrap();
    let b = args.get(1).int32_value(scope).unwrap();
    rv.set_int32(a.wrapping_add(b));
}

// ---- print(pointer, length): reads guest memory.

fn print_fast(
    _receiver: v8::Local<v8::Value>,
    pointer: u32,
    length: u32,
    options: &fast_api::FastApiCallbackOptions,
) {
    let state = state(options);
    let (start, end) = (pointer as usize, pointer as usize + length as usize);
    assert!(end <= state.memory_len);
    let bytes = unsafe { std::slice::from_raw_parts(state.memory.add(start), end - start) };
    state
        .printed
        .lock()
        .unwrap()
        .push(String::from_utf8_lossy(bytes).into_owned());
}

const PRINT_FAST: CFunction = CFunction::new(
    print_fast as *const c_void,
    &CFunctionInfo::new(
        Type::Void.as_info(),
        &[
            Type::V8Value.as_info(),
            Type::Uint32.as_info(),
            Type::Uint32.as_info(),
            Type::CallbackOptions.as_info(),
        ],
        Int64Representation::BigInt,
    ),
);

// ---- fail(code): a native import that stops the guest by throwing.

fn fail_fast(_receiver: v8::Local<v8::Value>, code: i32, options: &fast_api::FastApiCallbackOptions) {
    v8::callback_scope!(unsafe scope, options);
    let message = v8::String::new(scope, &format!("the host refused a call (code {code})")).unwrap();
    let error = v8::Exception::error(scope, message);
    scope.throw_exception(error);
}

const FAIL_FAST: CFunction = CFunction::new(
    fail_fast as *const c_void,
    &CFunctionInfo::new(
        Type::Void.as_info(),
        &[
            Type::V8Value.as_info(),
            Type::Int32.as_info(),
            Type::CallbackOptions.as_info(),
        ],
        Int64Representation::BigInt,
    ),
);

// ---- mix(i64, f64, f32) -> f64: every other wasm value type.

fn mix_fast(
    _receiver: v8::Local<v8::Value>,
    a: i64,
    b: f64,
    c: f32,
    _options: &fast_api::FastApiCallbackOptions,
) -> f64 {
    a as f64 + b + c as f64
}

const MIX_FAST: CFunction = CFunction::new(
    mix_fast as *const c_void,
    &CFunctionInfo::new(
        Type::Float64.as_info(),
        &[
            Type::V8Value.as_info(),
            Type::Int64.as_info(),
            Type::Float64.as_info(),
            Type::Float32.as_info(),
            Type::CallbackOptions.as_info(),
        ],
        Int64Representation::BigInt,
    ),
);

fn unreachable_slow(
    scope: &mut v8::PinScope,
    _args: v8::FunctionCallbackArguments,
    _rv: v8::ReturnValue<v8::Value>,
) {
    let message = v8::String::new(scope, "a fast import took the slow path").unwrap();
    let error = v8::Exception::error(scope, message);
    scope.throw_exception(error);
}

// ---- helpers over the WebAssembly built-ins (called from Rust, no JS source).

fn web_assembly<'s>(scope: &mut v8::PinScope<'s, '_>, name: &str) -> v8::Local<'s, v8::Function> {
    let global = scope.get_current_context().global(scope);
    let key = v8::String::new(scope, "WebAssembly").unwrap();
    let wasm: v8::Local<v8::Object> = global.get(scope, key.into()).unwrap().try_into().unwrap();
    let key = v8::String::new(scope, name).unwrap();
    wasm.get(scope, key.into()).unwrap().try_into().unwrap()
}

fn set<'s>(scope: &mut v8::PinScope<'s, '_>, object: v8::Local<v8::Object>, key: &str, value: v8::Local<v8::Value>) {
    let key = v8::String::new(scope, key).unwrap();
    object.set(scope, key.into(), value).unwrap();
}

fn get<'s>(scope: &mut v8::PinScope<'s, '_>, object: v8::Local<v8::Object>, key: &str) -> v8::Local<'s, v8::Value> {
    let key = v8::String::new(scope, key).unwrap();
    object.get(scope, key.into()).unwrap()
}

fn shared_memory<'s>(scope: &mut v8::PinScope<'s, '_>) -> v8::Local<'s, v8::Object> {
    let descriptor = v8::Object::new(scope);
    let one = v8::Integer::new(scope, 1).into();
    let hundred = v8::Integer::new(scope, 100).into();
    let yes = v8::Boolean::new(scope, true).into();
    set(scope, descriptor, "initial", one);
    set(scope, descriptor, "maximum", hundred);
    set(scope, descriptor, "shared", yes);
    let constructor = web_assembly(scope, "Memory");
    constructor.new_instance(scope, &[descriptor.into()]).unwrap()
}

fn import<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    data: v8::Local<'s, v8::Value>,
    slow: impl v8::MapFnTo<v8::FunctionCallback>,
    fast: Option<&'static [CFunction]>,
) -> v8::Local<'s, v8::Value> {
    let builder = v8::FunctionTemplate::builder(slow).data(data);
    let template = match fast {
        Some(overloads) => builder.build_fast(scope, overloads),
        None => builder.build(scope),
    };
    template.get_function(scope).unwrap().into()
}

fn imports<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    memory: v8::Local<'s, v8::Object>,
    state: &HostState,
) -> v8::Local<'s, v8::Object> {
    let data = v8::External::new(scope, state as *const HostState as *mut c_void).into();
    let host = v8::Object::new(scope);
    let add = import(scope, data, unreachable_slow, Some(&[ADD_FAST]));
    let add_slow_function = import(scope, data, add_slow, None);
    let print = import(scope, data, unreachable_slow, Some(&[PRINT_FAST]));
    let fail = import(scope, data, unreachable_slow, Some(&[FAIL_FAST]));
    let mix = import(scope, data, unreachable_slow, Some(&[MIX_FAST]));
    set(scope, host, "add", add);
    set(scope, host, "add_slow", add_slow_function);
    set(scope, host, "print", print);
    set(scope, host, "fail", fail);
    set(scope, host, "mix", mix);
    let env = v8::Object::new(scope);
    set(scope, env, "memory", memory.into());
    let imports = v8::Object::new(scope);
    set(scope, imports, "host", host.into());
    set(scope, imports, "env", env.into());
    imports
}

fn instantiate<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    module: v8::Local<'s, v8::WasmModuleObject>,
    imports: v8::Local<'s, v8::Object>,
) -> v8::Local<'s, v8::Object> {
    let constructor = web_assembly(scope, "Instance");
    let instance = constructor
        .new_instance(scope, &[module.into(), imports.into()])
        .expect("instantiate");
    get(scope, instance, "exports").try_into().unwrap()
}

fn export<'s>(scope: &mut v8::PinScope<'s, '_>, exports: v8::Local<v8::Object>, name: &str) -> v8::Local<'s, v8::Function> {
    get(scope, exports, name).try_into().unwrap()
}

fn memory_base(scope: &mut v8::PinScope, memory: v8::Local<v8::Object>) -> (*const u8, usize) {
    let buffer: v8::Local<v8::SharedArrayBuffer> = get(scope, memory, "buffer").try_into().unwrap();
    let store = buffer.get_backing_store();
    (store.data().unwrap().as_ptr() as *const u8, store.byte_length())
}

fn call_i32(scope: &mut v8::PinScope, function: v8::Local<v8::Function>, argument: i32) -> i32 {
    let receiver = v8::undefined(scope).into();
    let argument = v8::Integer::new(scope, argument).into();
    function.call(scope, receiver, &[argument]).unwrap().int32_value(scope).unwrap()
}

/// Shares one SharedArrayBuffer's backing store with another isolate through
/// V8's value serializer, the way postMessage moves a shared WebAssembly.Memory.
struct Transfer {
    stores: Mutex<Vec<v8::SharedRef<v8::BackingStore>>>,
}

impl v8::ValueSerializerImpl for Transfer {
    fn throw_data_clone_error<'s>(&self, scope: &mut v8::PinScope<'s, '_>, message: v8::Local<'s, v8::String>) {
        let error = v8::Exception::error(scope, message);
        scope.throw_exception(error);
    }

    fn get_shared_array_buffer_id<'s>(
        &self,
        _scope: &mut v8::PinScope<'s, '_>,
        buffer: v8::Local<'s, v8::SharedArrayBuffer>,
    ) -> Option<u32> {
        let mut stores = self.stores.lock().unwrap();
        stores.push(buffer.get_backing_store());
        Some(stores.len() as u32 - 1)
    }
}

impl v8::ValueDeserializerImpl for Transfer {
    fn get_shared_array_buffer_from_id<'s>(
        &self,
        scope: &mut v8::PinScope<'s, '_>,
        id: u32,
    ) -> Option<v8::Local<'s, v8::SharedArrayBuffer>> {
        let store = self.stores.lock().unwrap()[id as usize].clone();
        Some(v8::SharedArrayBuffer::with_backing_store(scope, &store))
    }
}

fn new_isolate() -> v8::OwnedIsolate {
    v8::Isolate::new(v8::CreateParams::default())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("compile") {
        return compile_only(&args[1..]);
    }
    if args.first().map(String::as_str) == Some("bench") {
        return bench(&args[1]);
    }
    let big = args.first().cloned();
    // The flag that lets a wasm import call a FunctionTemplate's C function
    // directly. Off by default: V8 cannot check the receiver for wasm.
    v8::V8::set_flags_from_string("--wasm-unsafe-fast-api-wrapper");
    let platform = v8::new_default_platform(0, false).make_shared();
    v8::V8::initialize_platform(platform);
    v8::V8::initialize();

    let wire = wat::parse_str(GUEST).unwrap();
    let mut isolate = new_isolate();
    let handle = isolate.thread_safe_handle();
    v8::scope!(let scope, &mut isolate);
    let context = v8::Context::new(scope, Default::default());
    let scope = &mut v8::ContextScope::new(scope, context);

    // 1. compile and instantiate
    let started = Instant::now();
    let module = v8::WasmModuleObject::compile(scope, &wire).expect("compile");
    let memory = shared_memory(scope);
    let (base, len) = memory_base(scope, memory);
    let host = Box::leak(Box::new(HostState {
        memory: base,
        memory_len: len,
        printed: Mutex::default(),
        fast_calls: AtomicU64::new(0),
        slow_calls: AtomicU64::new(0),
    }));
    let imported = imports(scope, memory, host);
    let exports = instantiate(scope, module, imported);
    println!("1. compiled and instantiated the guest in {:?}", started.elapsed());

    // 2. a native import reading guest memory
    let hello = export(scope, exports, "hello");
    let receiver = v8::undefined(scope).into();
    hello.call(scope, receiver, &[]).unwrap();
    println!("2. print import read: {:?}", host.printed.lock().unwrap());

    // 3. per-call cost
    const N: i32 = 20_000_000;
    for name in ["loop_wasm", "loop_fast", "loop_slow"] {
        let function = export(scope, exports, name);
        let n = if name == "loop_slow" { N / 10 } else { N };
        call_i32(scope, function, 1000); // warm up and tier up
        call_i32(scope, function, n);
        let started = Instant::now();
        let result = call_i32(scope, function, n);
        let took = started.elapsed();
        println!(
            "3. {name}: {n} calls in {took:?}, {:.2} ns per call (result {result})",
            took.as_nanos() as f64 / n as f64
        );
    }
    println!(
        "   fast callbacks run: {}, slow callbacks run: {}",
        host.fast_calls.load(Ordering::Relaxed),
        host.slow_calls.load(Ordering::Relaxed)
    );
    let mix = export(scope, exports, "mix");
    let receiver = v8::undefined(scope).into();
    let mixed = mix.call(scope, receiver, &[]).unwrap().number_value(scope).unwrap();
    println!("   mix(i64 0x100000001, f64 0.5, f32 0.25) = {mixed} (expect {})", 0x100000001u64 as f64 + 0.75);

    // 4. a native import that throws, and wasm's own exceptions
    {
        v8::tc_scope!(let tc, scope);
        let fail = export(tc, exports, "call_fail");
        let receiver = v8::undefined(tc).into();
        let result = fail.call(tc, receiver, &[]);
        let message = tc.message().map(|m| m.get(tc).to_rust_string_lossy(tc));
        let stack = tc.stack_trace().map(|s| s.to_rust_string_lossy(tc));
        println!("4. call_fail returned {:?}; caught: {message:?}", result.map(|_| ()));
        println!("   stack: {}", stack.unwrap_or_default().replace('\n', " | "));
    }
    let catch_own = export(scope, exports, "catch_own");
    println!("   catch_own (try_table/throw, exnref) = {}", call_i32(scope, catch_own, 0));

    // 5. TerminateExecution from another thread
    for name in ["spin", "wait"] {
        let function = export(scope, exports, name);
        let interrupter = handle.clone();
        let stopper = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(200));
            let started = Instant::now();
            interrupter.terminate_execution();
            started
        });
        v8::tc_scope!(let tc, scope);
        let receiver = v8::undefined(tc).into();
        let result = tc_call(tc, function, receiver);
        let asked = stopper.join().unwrap();
        println!(
            "5. {name}: returned {:?} {:?} after terminate; terminating: {}",
            result,
            asked.elapsed(),
            tc.is_execution_terminating()
        );
        tc.cancel_terminate_execution();
    }

    // 6. a second isolate on another thread: same compiled code, same memory
    {
        let compiled = module.get_compiled_module();
        let transfer = Arc::new(Transfer { stores: Mutex::default() });
        let bytes = {
            let serializer = v8::ValueSerializer::new(scope, Box::new(SharedTransfer(Arc::clone(&transfer))));
            serializer.write_header();
            let context = scope.get_current_context();
            serializer.write_value(context, memory.into()).expect("serialize memory");
            serializer.release()
        };
        let (sender, receiver) = mpsc::channel();
        let worker_transfer = Arc::clone(&transfer);
        let worker = std::thread::spawn(move || {
            let started = Instant::now();
            let mut isolate = new_isolate();
            v8::scope!(let scope, &mut isolate);
            let context = v8::Context::new(scope, Default::default());
            let scope = &mut v8::ContextScope::new(scope, context);
            let deserializer = v8::ValueDeserializer::new(scope, Box::new(SharedTransfer(worker_transfer)), &bytes);
            let context = scope.get_current_context();
            deserializer.read_header(context).unwrap();
            let memory: v8::Local<v8::Object> = deserializer.read_value(context).unwrap().try_into().unwrap();
            let module = v8::WasmModuleObject::from_compiled_module(scope, &compiled).unwrap();
            let (base, len) = memory_base(scope, memory);
            let host = Box::leak(Box::new(HostState {
                memory: base,
                memory_len: len,
                printed: Mutex::default(),
                fast_calls: AtomicU64::new(0),
                slow_calls: AtomicU64::new(0),
            }));
            let imported = imports(scope, memory, host);
            let exports = instantiate(scope, module, imported);
            let ready = started.elapsed();
            let store = export(scope, exports, "store_and_notify");
            call_i32(scope, store, 1234);
            sender.send((ready, base as usize)).unwrap();
        });
        let wait = export(scope, exports, "wait_for_store");
        let value = call_i32(scope, wait, 0);
        let (ready, other_base) = receiver.recv().unwrap();
        worker.join().unwrap();
        println!(
            "6. second isolate ready in {ready:?} (same memory: {}); main thread woke with {value}",
            other_base == base as usize
        );
    }

    // 7. a big module
    if let Some(path) = big {
        let bytes = std::fs::read(&path).unwrap();
        for _ in 0..2 {
            let started = Instant::now();
            let module = v8::WasmModuleObject::compile(scope, &bytes);
            println!(
                "7. {path} ({:.1} MB): WasmModuleObject::compile {:?} ({})",
                bytes.len() as f64 / 1e6,
                started.elapsed(),
                if module.is_some() { "ok" } else { "failed" }
            );
        }
    }
}

struct SharedTransfer(Arc<Transfer>);

impl v8::ValueSerializerImpl for SharedTransfer {
    fn throw_data_clone_error<'s>(&self, scope: &mut v8::PinScope<'s, '_>, message: v8::Local<'s, v8::String>) {
        self.0.throw_data_clone_error(scope, message)
    }

    fn get_shared_array_buffer_id<'s>(
        &self,
        scope: &mut v8::PinScope<'s, '_>,
        buffer: v8::Local<'s, v8::SharedArrayBuffer>,
    ) -> Option<u32> {
        self.0.get_shared_array_buffer_id(scope, buffer)
    }
}

impl v8::ValueDeserializerImpl for SharedTransfer {
    fn get_shared_array_buffer_from_id<'s>(
        &self,
        scope: &mut v8::PinScope<'s, '_>,
        id: u32,
    ) -> Option<v8::Local<'s, v8::SharedArrayBuffer>> {
        self.0.get_shared_array_buffer_from_id(scope, id)
    }
}

fn tc_call(
    scope: &mut v8::PinScope,
    function: v8::Local<v8::Function>,
    receiver: v8::Local<v8::Value>,
) -> Option<()> {
    function.call(scope, receiver, &[]).map(|_| ())
}

/// Times compiling each module given, on one isolate. Flags from V8_FLAGS.
fn compile_only(paths: &[String]) {
    let flags = std::env::var("V8_FLAGS").unwrap_or_default();
    v8::V8::set_flags_from_string(&flags);
    let platform = v8::new_default_platform(0, false).make_shared();
    v8::V8::initialize_platform(platform);
    v8::V8::initialize();
    let mut isolate = new_isolate();
    v8::scope!(let scope, &mut isolate);
    let context = v8::Context::new(scope, Default::default());
    let scope = &mut v8::ContextScope::new(scope, context);
    let cpu = || {
        let mut usage: libc_rusage = unsafe { std::mem::zeroed() };
        unsafe { getrusage(0, &mut usage) };
        let t = |s: i64, u: i64| s as f64 + u as f64 / 1e6;
        t(usage.utime_sec, usage.utime_usec) + t(usage.stime_sec, usage.stime_usec)
    };
    for path in paths {
        let bytes = std::fs::read(path).unwrap();
        let cpu_before = cpu();
        let started = Instant::now();
        let module = v8::WasmModuleObject::compile(scope, &bytes);
        println!(
            "{path} ({:.1} MB) [{flags}]: {:?} wall, {:.2} s CPU so far in process ({})",
            bytes.len() as f64 / 1e6,
            started.elapsed(),
            cpu() - cpu_before,
            if module.is_some() { "ok" } else { "FAILED" }
        );
    }
}

#[repr(C)]
struct libc_rusage {
    utime_sec: i64,
    utime_usec: i64,
    stime_sec: i64,
    stime_usec: i64,
    rest: [i64; 14],
}

unsafe extern "C" {
    fn getrusage(who: i32, usage: *mut libc_rusage) -> i32;
}

/// Calls a module's `run(rounds)` export repeatedly, timing each call.
fn bench(path: &str) {
    v8::V8::set_flags_from_string(&std::env::var("V8_FLAGS").unwrap_or_default());
    let platform = v8::new_default_platform(0, false).make_shared();
    v8::V8::initialize_platform(platform);
    v8::V8::initialize();
    let mut isolate = new_isolate();
    v8::scope!(let scope, &mut isolate);
    let context = v8::Context::new(scope, Default::default());
    let scope = &mut v8::ContextScope::new(scope, context);
    let bytes = std::fs::read(path).unwrap();
    let module = v8::WasmModuleObject::compile(scope, &bytes).unwrap();
    let imports = v8::Object::new(scope);
    let exports = instantiate(scope, module, imports);
    let run = export(scope, exports, "run");
    let mut times = Vec::new();
    for _ in 0..12 {
        let started = Instant::now();
        let receiver = v8::undefined(scope).into();
        let rounds = v8::Integer::new(scope, 20).into();
        run.call(scope, receiver, &[rounds]).unwrap();
        times.push(format!("{:.1}", started.elapsed().as_secs_f64() * 1e3));
    }
    println!("v8 run(20) ms per call: {}", times.join(" "));
}
