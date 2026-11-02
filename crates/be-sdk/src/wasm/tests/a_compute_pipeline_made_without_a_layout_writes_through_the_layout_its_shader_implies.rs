use wgpu::util::DeviceExt;

use super::*;

const SHADER: &str = "
@group(0) @binding(0) var<storage, read> input: array<u32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;

@compute @workgroup_size(4)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    output[id.x] = input[id.x] * input[id.x] + 1u;
}
";

#[test]
fn a_compute_pipeline_made_without_a_layout_writes_through_the_layout_its_shader_implies() {
    let (device, queue) = gpu();
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl(SHADER.into()),
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("squares"),
        layout: None,
        module: &module,
        entry_point: None,
        compilation_options: Default::default(),
        cache: None,
    });
    let numbers: Vec<u32> = (0..8).collect();
    let input = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("input"),
        contents: &numbers
            .iter()
            .flat_map(|number| number.to_le_bytes())
            .collect::<Vec<u8>>(),
        usage: wgpu::BufferUsages::STORAGE,
    });
    let size = 4 * numbers.len() as u64;
    let output = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("output"),
        size,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: input.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: output.as_entire_binding(),
            },
        ],
    });
    let destination = readable(&device, size);
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor::default());
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &group, &[]);
        pass.dispatch_workgroups(2, 1, 1);
    }
    encoder.copy_buffer_to_buffer(&output, 0, &destination, 0, size);
    queue.submit([encoder.finish()]);
    let read = read_back(&device, &destination);
    let squares: Vec<u32> = read
        .chunks_exact(4)
        .map(|word| u32::from_le_bytes(word.try_into().unwrap()))
        .collect();
    assert_eq!(squares, [1, 2, 5, 10, 17, 26, 37, 50]);
}
