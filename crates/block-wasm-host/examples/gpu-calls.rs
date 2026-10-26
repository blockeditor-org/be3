use std::time::{Duration, Instant};

use block_gpu_abi::device as abi;
use block_wasm_host::Host;

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
    let texture = data.put(be_ops::encode(&abi::TextureDescriptor {
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
    let view = data.put(be_ops::encode(&abi::TextureViewDescriptor {
        label: None,
        format: None,
        dimension: None,
        aspect: abi::TextureAspect::All,
        base_mip_level: 0,
        mip_level_count: None,
        base_array_layer: 0,
        array_layer_count: None,
    }));
    let module = data.put(be_ops::encode(&abi::ShaderModuleDescriptor {
        label: None,
        wgsl: SHADER.into(),
    }));
    let layout = data.put(be_ops::encode(&abi::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: Vec::new(),
    }));
    let pipeline = data.put(be_ops::encode(&abi::RenderPipelineDescriptor {
        label: None,
        layout: Some(abi::PipelineLayout(1)),
        vertex: abi::VertexState {
            module: abi::ShaderModule(1),
            entry_point: Some("vs_main".into()),
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
            targets: vec![Some(abi::ColorTargetState {
                format: abi::TextureFormat::Rgba8Unorm,
                blend: None,
                write_mask: 0xf,
            })],
        }),
    }));
    let encoder = data.put(be_ops::encode(&abi::CommandEncoderDescriptor {
        label: None,
    }));
    let pass = data.put(be_ops::encode(&abi::RenderPassDescriptor {
        label: None,
        color_attachments: vec![Some(abi::ColorAttachment {
            view: abi::TextureView(1),
            resolve_target: None,
            load: abi::ColorLoadOp::Load,
            store: abi::StoreOp::Store,
            depth_slice: None,
        })],
        depth_stencil_attachment: None,
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
            (global (export "__tls_size") i32 (i32.const 0))
            (global (export "__tls_align") i32 (i32.const 1))
            (func (export "plugin_initialize_tls") (param i32 i32))
            (func (export "plugin_start") {start})
            (func (export "plugin_shutdown"))
            (func (export "plugin_step") (local $left i32)
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
        GPU = block_gpu_abi::embed::device::MODULE,
    )
}

fn main() {
    let (device, queue) = wgpu::Device::noop(&wgpu::DeviceDescriptor::default());
    let host = Host::new(device, queue, None).expect("the host should start");
    let mut plugin = host
        .load_bytes(guest().as_bytes())
        .expect("the guest should load");
    plugin.start().expect("the guest should start");
    let mut fastest = Duration::MAX;
    let mut total = Duration::ZERO;
    for _ in 0..STEPS {
        let started = Instant::now();
        plugin.step().expect("the guest should step");
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
