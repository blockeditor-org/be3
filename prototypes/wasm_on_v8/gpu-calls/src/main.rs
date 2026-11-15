//! Step 1's gate for wasm on V8: crates/be-wasm-host's example-gpu-calls (the same guest, the
//! same 400k gpu import calls a step against a noop device) with every gpu import bound as a V8
//! fast-API function that calls the real glue in be-wasm-gpu-abi / be-wasm-gpu-host, as
//! be-wasm-host's `link_gpu` does on wasmtime.

#![allow(improper_ctypes_definitions)]

use std::collections::BTreeMap;
use std::ffi::c_void;
use std::time::{Duration, Instant};

use be_wasm_gpu_abi::device as abi;
use be_wasm_gpu_abi::embed::device::Caller;
use be_wasm_gpu_host::Gpu;
use v8::fast_api::{CFunction, CFunctionInfo, FastApiCallbackOptions, Int64Representation, Type};

const DRAWS: u32 = 100_000;
const STEPS: usize = 20;

const SHADER: &str = "
@vertex fn vs_main(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    return vec4<f32>(f32(index), 0.0, 0.0, 1.0);
}
@fragment fn fs_main() -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 0.0, 0.0, 1.0);
}
";

struct Data {
    bytes: Vec<u8>,
}

impl Data {
    fn put(&mut self, blob: Vec<u8>) -> (usize, usize) {
        let at = self.bytes.len();
        self.bytes.extend_from_slice(&blob);
        while !self.bytes.len().is_multiple_of(8) {
            self.bytes.push(0);
        }
        (at + 4096, blob.len())
    }
}

fn escaped(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("\\{byte:02x}")).collect()
}

fn guest() -> String {
    let mut data = Data { bytes: Vec::new() };
    let texture = data.put(be_wasm_ops::encode(&abi::TextureDescriptor {
        label: None,
        size: abi::Extent3d {
            width: 64,
            height: 64,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: abi::TextureDimension::D2,
        format: abi::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT.bits(),
        view_formats: Vec::new(),
    }));
    let view = data.put(be_wasm_ops::encode(&abi::TextureViewDescriptor {
        label: None,
        format: None,
        dimension: None,
        usage: None,
        aspect: abi::TextureAspect::All,
        base_mip_level: 0,
        mip_level_count: None,
        base_array_layer: 0,
        array_layer_count: None,
    }));
    let module = data.put(be_wasm_ops::encode(&abi::ShaderModuleDescriptor {
        label: None,
        wgsl: SHADER.into(),
    }));
    let layout = data.put(be_wasm_ops::encode(&abi::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: Vec::new(),
    }));
    let pipeline = data.put(be_wasm_ops::encode(&abi::RenderPipelineDescriptor {
        label: None,
        layout: Some(abi::PipelineLayout(1)),
        vertex: abi::VertexState {
            module: abi::ShaderModule(1),
            entry_point: Some("vs_main".into()),
            constants: Vec::new(),
            buffers: Vec::new(),
        },
        primitive: abi::PrimitiveState {
            topology: abi::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: abi::FrontFace::Ccw,
            cull_mode: None,
            unclipped_depth: false,
            polygon_mode: abi::PolygonMode::Fill,
            conservative: false,
        },
        depth_stencil: None,
        multisample: abi::MultisampleState {
            count: 1,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        fragment: Some(abi::FragmentState {
            module: abi::ShaderModule(1),
            entry_point: Some("fs_main".into()),
            constants: Vec::new(),
            targets: vec![Some(abi::ColorTargetState {
                format: abi::TextureFormat::Rgba8Unorm,
                blend: None,
                write_mask: 0xf,
            })],
        }),
    }));
    let encoder = data.put(be_wasm_ops::encode(&abi::CommandEncoderDescriptor {
        label: None,
    }));
    let pass = data.put(be_wasm_ops::encode(&abi::RenderPassDescriptor {
        label: None,
        color_attachments: vec![Some(abi::ColorAttachment {
            view: abi::TextureView(1),
            resolve_target: None,
            load: abi::ColorLoadOp::Load,
            store: abi::StoreOp::Store,
            depth_slice: None,
        })],
        depth_stencil_attachment: None,
        occlusion_query_set: None,
        timestamp_writes: None,
    }));
    let call = |name: &str, this: Option<u32>, (at, length): (usize, usize)| {
        let this = this.map_or(String::new(), |this| format!("(i32.const {this}) "));
        format!("(drop (call ${name} {this}(i32.const {at}) (i32.const {length})))")
    };
    let start = [
        call("create_texture", None, texture),
        call("create_texture_view", Some(1), view),
        call("create_shader_module", None, module),
        call("create_pipeline_layout", None, layout),
        call("create_render_pipeline", None, pipeline),
        call("create_command_encoder", None, encoder),
        call("encoder_begin_render_pass", Some(1), pass),
    ]
    .join("\n");
    format!(
        r#"(module
            (import "env" "memory" (memory 1 4 shared))
            (import "{GPU}" "create-texture" (func $create_texture (param i32 i32) (result i32)))
            (import "{GPU}" "[method]texture.create-view" (func $create_texture_view (param i32 i32 i32) (result i32)))
            (import "{GPU}" "create-shader-module" (func $create_shader_module (param i32 i32) (result i32)))
            (import "{GPU}" "create-pipeline-layout" (func $create_pipeline_layout (param i32 i32) (result i32)))
            (import "{GPU}" "create-render-pipeline" (func $create_render_pipeline (param i32 i32) (result i32)))
            (import "{GPU}" "create-command-encoder" (func $create_command_encoder (param i32 i32) (result i32)))
            (import "{GPU}" "[method]command-encoder.begin-render-pass" (func $encoder_begin_render_pass (param i32 i32 i32) (result i32)))
            (import "{GPU}" "[method]render-pass.set-pipeline" (func $set_pipeline (param i32 i32)))
            (import "{GPU}" "[method]render-pass.set-viewport" (func $set_viewport (param i32 f32 f32 f32 f32 f32 f32)))
            (import "{GPU}" "[method]render-pass.set-scissor-rect" (func $set_scissor (param i32 i32 i32 i32 i32)))
            (import "{GPU}" "[method]render-pass.draw" (func $draw (param i32 i32 i32 i32 i32)))
            (data (i32.const 4096) "{data}")
            (func (export "app_start") {start})
            (func (export "app_shutdown"))
            (func (export "app_step") (local $left i32)
                (local.set $left (i32.const {DRAWS}))
                (loop $draws
                    (call $set_pipeline (i32.const 1) (i32.const 1))
                    (call $set_viewport (i32.const 1) (f32.const 0) (f32.const 0) (f32.const 64) (f32.const 64) (f32.const 0) (f32.const 1))
                    (call $set_scissor (i32.const 1) (i32.const 0) (i32.const 0) (i32.const 32) (i32.const 32))
                    (call $draw (i32.const 1) (i32.const 3) (i32.const 1) (i32.const 0) (i32.const 0))
                    (local.set $left (i32.sub (local.get $left) (i32.const 1)))
                    (br_if $draws (local.get $left))))
        )"#,
        data = escaped(&data.bytes),
        GPU = be_wasm_gpu_abi::embed::device::MODULE,
    )
}


/// What each import reaches through its template's data.
struct Host {
    memory: *mut u8,
    length: usize,
    gpu: Gpu,
    returned: Vec<u8>,
    error: Option<String>,
    gpu_time: Duration,
    submit_time: Duration,
}

struct Guest<'a>(&'a mut Host);

impl Guest<'_> {
    fn range(&self, pointer: u32, length: usize) -> Result<std::ops::Range<usize>, String> {
        let start = pointer as usize;
        let end = start.checked_add(length).ok_or("an app pointer overflowed")?;
        match end <= self.0.length {
            true => Ok(start..end),
            false => Err(format!("an app pointer ran past its memory: {start}..{end}")),
        }
    }
}

impl Caller for Guest<'_> {
    type Host = Gpu;

    fn read(&mut self, pointer: u32, length: u32) -> Result<Vec<u8>, String> {
        let range = self.range(pointer, length as usize)?;
        // A shared memory never moves: V8 reserves its maximum.
        let bytes = unsafe { std::slice::from_raw_parts(self.0.memory.add(range.start), range.len()) };
        Ok(bytes.to_vec())
    }

    fn write(&mut self, pointer: u32, bytes: &[u8]) -> Result<(), String> {
        let range = self.range(pointer, bytes.len())?;
        unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), self.0.memory.add(range.start), bytes.len()) };
        Ok(())
    }

    /// As be-wasm-host's `State::host`: the time spent in the gpu is accounted per call.
    #[inline]
    fn host<R>(&mut self, function: &'static str, call: impl FnOnce(&mut Gpu) -> R) -> Option<R> {
        let started = Instant::now();
        let result = call(&mut self.0.gpu);
        let took = started.elapsed();
        self.0.gpu_time += took;
        if function == "submit" {
            self.0.submit_time += took;
        }
        Some(result)
    }

    fn refuse(&mut self, message: String) {
        self.0.gpu.report(message);
    }

    fn returned(&mut self) -> &mut Vec<u8> {
        &mut self.0.returned
    }
}

trait Wasm {
    const TYPE: Type;
}

macro_rules! wasm_types {
    ($($rust:ty => $v8:ident),*) => { $(impl Wasm for $rust { const TYPE: Type = Type::$v8; })* };
}

wasm_types!(() => Void, i32 => Int32, u32 => Uint32, i64 => Int64, u64 => Uint64, f32 => Float32, f64 => Float64);

macro_rules! returns {
    () => { () };
    ($result:ty) => { $result };
}

fn host_of(options: &FastApiCallbackOptions) -> &'static mut Host {
    let external: v8::Local<v8::External> = options.data.try_into().unwrap();
    unsafe { &mut *(external.value() as *mut Host) }
}

fn never_slow(scope: &mut v8::PinScope, _: v8::FunctionCallbackArguments, _: v8::ReturnValue<v8::Value>) {
    let message = v8::String::new(scope, "a fast import took the slow path").unwrap();
    let error = v8::Exception::error(scope, message);
    scope.throw_exception(error);
}

macro_rules! bind_gpu {
    ($($glue:path, $module:literal, $name:literal, $export:literal, ($($argument:ident: $kind:ty),*) $(-> $result:ty)?;)*) => {
        /// Every gpu import, by module and name, as a fast-API function over `data`.
        fn gpu_imports<'s>(
            scope: &mut v8::PinScope<'s, '_>,
            data: v8::Local<'s, v8::Value>,
        ) -> Vec<(&'static str, &'static str, v8::Local<'s, v8::Value>)> {
            let mut imports = Vec::new();
            $({
                extern "C" fn fast(
                    _receiver: v8::Local<v8::Value>,
                    $($argument: $kind,)*
                    options: &FastApiCallbackOptions,
                ) -> returns!($($result)?) {
                    let host = host_of(options);
                    $glue(&mut Guest(host), $($argument),*)
                }
                const INFO: CFunctionInfo = CFunctionInfo::new(
                    <returns!($($result)?) as Wasm>::TYPE.as_info(),
                    &[Type::V8Value.as_info(), $(<$kind as Wasm>::TYPE.as_info(),)* Type::CallbackOptions.as_info()],
                    Int64Representation::BigInt,
                );
                const FAST: [CFunction; 1] = [CFunction::new(fast as *const c_void, &INFO)];
                let template = v8::FunctionTemplate::builder(never_slow).data(data).build_fast(scope, &FAST);
                imports.push(($module, $name, template.get_function(scope).unwrap().into()));
            })*
            imports
        }
    };
}

be_wasm_gpu_abi::gpu_local_imports!(bind_gpu);

fn key<'s>(scope: &mut v8::PinScope<'s, '_>, name: &str) -> v8::Local<'s, v8::Value> {
    v8::String::new(scope, name).unwrap().into()
}

fn web_assembly<'s>(scope: &mut v8::PinScope<'s, '_>, name: &str) -> v8::Local<'s, v8::Function> {
    let global = scope.get_current_context().global(scope);
    let wasm_key = key(scope, "WebAssembly");
    let wasm: v8::Local<v8::Object> = global.get(scope, wasm_key).unwrap().try_into().unwrap();
    let name = key(scope, name);
    wasm.get(scope, name).unwrap().try_into().unwrap()
}

fn main() {
    v8::V8::set_flags_from_string(&std::env::var("V8_FLAGS").map(|flags| format!("--wasm-unsafe-fast-api-wrapper {flags}")).unwrap_or_else(|_| "--wasm-unsafe-fast-api-wrapper".to_owned()));
    let platform = v8::new_default_platform(0, false).make_shared();
    v8::V8::initialize_platform(platform);
    v8::V8::initialize();

    let wire = wat::parse_str(guest()).expect("the guest assembles");
    let mut isolate = v8::Isolate::new(v8::CreateParams::default());
    v8::scope!(let scope, &mut isolate);
    let context = v8::Context::new(scope, Default::default());
    let scope = &mut v8::ContextScope::new(scope, context);

    let module = v8::WasmModuleObject::compile(scope, &wire).expect("the guest compiles");
    let descriptor = v8::Object::new(scope);
    for (name, value) in [("initial", 1), ("maximum", 4)] {
        let name = key(scope, name);
        let value = v8::Integer::new(scope, value).into();
        descriptor.set(scope, name, value).unwrap();
    }
    let shared = key(scope, "shared");
    let yes = v8::Boolean::new(scope, true).into();
    descriptor.set(scope, shared, yes).unwrap();
    let memory_constructor = web_assembly(scope, "Memory");
    let memory = memory_constructor.new_instance(scope, &[descriptor.into()]).unwrap();
    let buffer_key = key(scope, "buffer");
    let buffer: v8::Local<v8::SharedArrayBuffer> = memory.get(scope, buffer_key).unwrap().try_into().unwrap();
    let store = buffer.get_backing_store();
    let (device, queue) = wgpu::Device::noop(&wgpu::DeviceDescriptor::default());
    let host = Box::leak(Box::new(Host {
        memory: store.data().unwrap().as_ptr() as *mut u8,
        length: store.byte_length(),
        gpu: Gpu::new(device, queue),
        returned: Vec::new(),
        error: None,
        gpu_time: Duration::ZERO,
        submit_time: Duration::ZERO,
    }));
    let data = v8::External::new(scope, host as *mut Host as *mut c_void).into();

    let mut modules: BTreeMap<&str, v8::Local<v8::Object>> = BTreeMap::new();
    for (module_name, name, function) in gpu_imports(scope, data) {
        let object = *modules.entry(module_name).or_insert_with(|| v8::Object::new(scope));
        let name = key(scope, name);
        object.set(scope, name, function).unwrap();
    }
    let env = v8::Object::new(scope);
    let memory_key = key(scope, "memory");
    env.set(scope, memory_key, memory.into()).unwrap();
    let imports = v8::Object::new(scope);
    let env_key = key(scope, "env");
    imports.set(scope, env_key, env.into()).unwrap();
    for (name, object) in modules {
        let name = key(scope, name);
        imports.set(scope, name, object.into()).unwrap();
    }
    let instance_constructor = web_assembly(scope, "Instance");
    let instance = instance_constructor
        .new_instance(scope, &[module.into(), imports.into()])
        .expect("the guest instantiates");
    let exports_key = key(scope, "exports");
    let exports: v8::Local<v8::Object> = instance.get(scope, exports_key).unwrap().try_into().unwrap();
    let export = |scope: &mut v8::PinScope, name: &str| -> v8::Global<v8::Function> {
        let name = key(scope, name);
        let function: v8::Local<v8::Function> = exports.get(scope, name).unwrap().try_into().unwrap();
        v8::Global::new(scope, function)
    };
    let start = export(scope, "app_start");
    let step = export(scope, "app_step");
    let receiver = v8::undefined(scope).into();
    v8::Local::new(scope, &start).call(scope, receiver, &[]).expect("the guest starts");
    if let Some(error) = host.gpu.take_error().or(host.error.take()) {
        panic!("the guest's start was refused: {error}");
    }
    let mut fastest = Duration::MAX;
    let mut total = Duration::ZERO;
    for _ in 0..STEPS {
        let started = Instant::now();
        v8::Local::new(scope, &step).call(scope, receiver, &[]).expect("the guest steps");
        let took = started.elapsed();
        fastest = fastest.min(took);
        total += took;
    }
    if let Some(error) = host.gpu.take_error() {
        panic!("a draw was refused: {error}");
    }
    let calls = f64::from(DRAWS * 4);
    eprintln!("gpu time {:?}, submit time {:?}", host.gpu_time, host.submit_time);
    println!(
        "{DRAWS} x (set pipeline, set viewport, set scissor, draw), {STEPS} steps on V8: \
         fastest {:.1} ns per call, mean {:.1} ns per call",
        fastest.as_nanos() as f64 / calls,
        total.as_nanos() as f64 / calls / STEPS as f64,
    );
}
