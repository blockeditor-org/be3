use super::*;

const SHADER: &str = "
override scale: u32 = 1u;
@id(7) override offset: f32 = 0.0;

@group(0) @binding(0) var<storage, read_write> output: array<u32>;

@compute @workgroup_size(4)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    output[id.x] = id.x * scale + u32(offset);
}
";

#[test]
fn a_compute_pipeline_writes_the_values_given_to_its_overridable_constants() {
    let (device, queue) = gpu();
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl(SHADER.into()),
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("scaled"),
        layout: None,
        module: &module,
        entry_point: None,
        compilation_options: wgpu::PipelineCompilationOptions {
            constants: &[("scale", 10.0), ("7", 3.0)],
            ..Default::default()
        },
        cache: None,
    });
    let size = 4 * 4;
    let output = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("output"),
        size,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: output.as_entire_binding(),
        }],
    });
    let destination = readable(&device, size);
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor::default());
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &group, &[]);
        pass.dispatch_workgroups(1, 1, 1);
    }
    encoder.copy_buffer_to_buffer(&output, 0, &destination, 0, size);
    queue.submit([encoder.finish()]);
    let read = read_back(&device, &destination);
    let written: Vec<u32> = read
        .chunks_exact(4)
        .map(|word| u32::from_le_bytes(word.try_into().unwrap()))
        .collect();
    assert_eq!(written, [3, 13, 23, 33]);
}
