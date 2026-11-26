use std::time::{Duration, Instant};

use be_wasm_gpu_abi::webgpu::webgpu as web;
use be_wasm_host::Host;

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

/// The guest, importing its memory, unshared as every guest's is
/// (decisions/guests_are_single_threaded.md). Its gpu imports are fast-API calls where the host's
/// memories stay put (V8's trap handler is on), and ordinary V8 calls where they may not. It asks
/// for its adapter and device through `[poll]`, and each handle it is given is the first of its
/// kind, 1, which the descriptors name.
fn guest() -> String {
    let mut data = Data { bytes: Vec::new() };
    let none = data.put(vec![0]);
    let texture = data.put(be_wasm_ops::encode(&web::GpuTextureDescriptor {
        size: web::GpuExtent3D {
            width: 64,
            height: Some(64),
            depth_or_array_layers: None,
        },
        mip_level_count: None,
        sample_count: None,
        dimension: None,
        format: web::GpuTextureFormat::Rgba8unorm,
        usage: web::GpuTextureUsage::RENDER_ATTACHMENT,
        view_formats: None,
        texture_binding_view_dimension: None,
        label: None,
    }));
    let module = data.put(be_wasm_ops::encode(&web::GpuShaderModuleDescriptor {
        code: SHADER.to_owned(),
        compilation_hints: None,
        label: None,
    }));
    let layout = data.put(be_wasm_ops::encode(&web::GpuPipelineLayoutDescriptor {
        bind_group_layouts: Vec::new(),
        immediate_size: None,
        label: None,
    }));
    let stage = |entry_point: &str| (web::GpuShaderModule(1), Some(entry_point.to_owned()));
    let pipeline = data.put(be_wasm_ops::encode(&web::GpuRenderPipelineDescriptor {
        vertex: web::GpuVertexState {
            buffers: None,
            module: stage("vs_main").0,
            entry_point: stage("vs_main").1,
            constants: None,
        },
        primitive: None,
        depth_stencil: None,
        multisample: None,
        fragment: Some(web::GpuFragmentState {
            targets: vec![Some(web::GpuColorTargetState {
                format: web::GpuTextureFormat::Rgba8unorm,
                blend: None,
                write_mask: None,
            })],
            module: stage("fs_main").0,
            entry_point: stage("fs_main").1,
            constants: None,
        }),
        layout: web::GpuLayoutMode::Specific(web::GpuPipelineLayout(1)),
        label: None,
    }));
    let pass = data.put(be_wasm_ops::encode(&web::GpuRenderPassDescriptor {
        color_attachments: vec![Some(web::GpuRenderPassColorAttachment {
            view: web::GpuTextureView(1),
            depth_slice: None,
            resolve_target: None,
            clear_value: None,
            load_op: web::GpuLoadOp::Load,
            store_op: web::GpuStoreOp::Store,
        })],
        depth_stencil_attachment: None,
        occlusion_query_set: None,
        timestamp_writes: None,
        max_draw_count: None,
        label: None,
    }));
    let call = |name: &str, this: u32, (at, length): (usize, usize)| {
        format!("(drop (call ${name} (i32.const {this}) (i32.const {at}) (i32.const {length})))")
    };
    let start = [
        call("request_adapter", 1, none),
        "(drop (call $poll (i32.const 1) (i32.const 0) (i32.const 4096)))".to_owned(),
        call("request_device", 1, none),
        "(drop (call $poll (i32.const 1) (i32.const 0) (i32.const 4096)))".to_owned(),
        call("create_texture", 1, texture),
        call("create_texture_view", 1, none),
        call("create_shader_module", 1, module),
        call("create_pipeline_layout", 1, layout),
        call("create_render_pipeline", 1, pipeline),
        call("create_command_encoder", 1, none),
        call("encoder_begin_render_pass", 1, pass),
    ]
    .join("\n");
    format!(
        r#"(module
            (import "env" "memory" (memory 1 4))
            (import "{GPU}" "get-gpu" (func $get_gpu (result i32)))
            (import "{GPU}" "[method]gpu.request-adapter" (func $request_adapter (param i32 i32 i32) (result i32)))
            (import "{GPU}" "[method]gpu-adapter.request-device" (func $request_device (param i32 i32 i32) (result i32)))
            (import "{GPU}" "[poll]" (func $poll (param i32 i32 i32) (result i32)))
            (import "{GPU}" "[method]gpu-device.create-texture" (func $create_texture (param i32 i32 i32) (result i32)))
            (import "{GPU}" "[method]gpu-texture.create-view" (func $create_texture_view (param i32 i32 i32) (result i32)))
            (import "{GPU}" "[method]gpu-device.create-shader-module" (func $create_shader_module (param i32 i32 i32) (result i32)))
            (import "{GPU}" "[method]gpu-device.create-pipeline-layout" (func $create_pipeline_layout (param i32 i32 i32) (result i32)))
            (import "{GPU}" "[method]gpu-device.create-render-pipeline" (func $create_render_pipeline (param i32 i32 i32) (result i32)))
            (import "{GPU}" "[method]gpu-device.create-command-encoder" (func $create_command_encoder (param i32 i32 i32) (result i32)))
            (import "{GPU}" "[method]gpu-command-encoder.begin-render-pass" (func $encoder_begin_render_pass (param i32 i32 i32) (result i32)))
            (import "{GPU}" "[method]gpu-render-pass-encoder.set-pipeline" (func $set_pipeline (param i32 i32)))
            (import "{GPU}" "[method]gpu-render-pass-encoder.set-viewport" (func $set_viewport (param i32 f32 f32 f32 f32 f32 f32)))
            (import "{GPU}" "[method]gpu-render-pass-encoder.set-scissor-rect" (func $set_scissor (param i32 i32 i32 i32 i32)))
            (import "{GPU}" "[method]gpu-render-pass-encoder.draw" (func $draw (param i32 i32 i32 i32 i32 i32 i32 i32)))
            (data (i32.const 4096) "{data}")
            (func (export "app_start") (drop (call $get_gpu)) {start})
            (func (export "app_shutdown"))
            (func (export "app_step") (local $left i32)
                (local.set $left (i32.const {DRAWS}))
                (loop $draws
                    (call $set_pipeline (i32.const 1) (i32.const 1))
                    (call $set_viewport (i32.const 1) (f32.const 0) (f32.const 0) (f32.const 64) (f32.const 64) (f32.const 0) (f32.const 1))
                    (call $set_scissor (i32.const 1) (i32.const 0) (i32.const 0) (i32.const 32) (i32.const 32))
                    (call $draw (i32.const 1) (i32.const 3) (i32.const 1) (i32.const 1) (i32.const 1) (i32.const 0) (i32.const 1) (i32.const 0))
                    (local.set $left (i32.sub (local.get $left) (i32.const 1)))
                    (br_if $draws (local.get $left))))
        )"#,
        data = escaped(&data.bytes),
        GPU = be_wasm_gpu_abi::embed::webgpu::MODULE,
    )
}

fn main() {
    let (device, queue) = wgpu::Device::noop(&wgpu::DeviceDescriptor::default());
    let host = Host::new(device, queue).expect("the host should start");
    let mut app = host
        .load_bytes(&wat::parse_str(guest()).expect("the guest should assemble"))
        .expect("the guest should load");
    app.start().expect("the guest should start");
    app.step().expect("the guest should step");
    let logged = app.take_logged();
    assert!(
        logged.is_empty(),
        "the guest's calls should be answered: {logged:?}"
    );
    let mut fastest = Duration::MAX;
    let mut total = Duration::ZERO;
    for _ in 0..STEPS {
        let started = Instant::now();
        app.step().expect("the guest should step");
        let took = started.elapsed();
        fastest = fastest.min(took);
        total += took;
    }
    let calls = f64::from(DRAWS * 4);
    println!(
        "{DRAWS} x (set pipeline, set viewport, set scissor, draw), {STEPS} steps: \
         fastest {:.1} ns per call, mean {:.1} ns per call",
        fastest.as_nanos() as f64 / calls,
        total.as_nanos() as f64 / calls / STEPS as f64,
    );
}
