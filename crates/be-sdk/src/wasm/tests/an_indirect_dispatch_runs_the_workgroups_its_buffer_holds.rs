use wgpu::util::DeviceExt;

use super::*;

const SHADER: &str = "
@group(0) @binding(0) var<storage, read_write> output: array<u32>;

@compute @workgroup_size(4)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    output[id.x] = id.x + 1u;
}
";

const WORDS: u64 = 16;

#[test]
fn an_indirect_dispatch_runs_the_workgroups_its_buffer_holds() {
    let (device, queue) = gpu();
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl(SHADER.into()),
    });
    let group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: None,
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only: false },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[Some(&group_layout)],
        immediate_size: 0,
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("counting"),
        layout: Some(&layout),
        module: &module,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    });
    let output = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("output"),
        size: 4 * WORDS,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &group_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: output.as_entire_binding(),
        }],
    });
    let counts = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("workgroup counts"),
        contents: &[7u32, 3, 1, 1]
            .iter()
            .flat_map(|count| count.to_le_bytes())
            .collect::<Vec<u8>>(),
        usage: wgpu::BufferUsages::INDIRECT,
    });
    let destination = readable(&device, 4 * WORDS);
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor::default());
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &group, &[]);
        pass.dispatch_workgroups_indirect(&counts, 4);
    }
    encoder.copy_buffer_to_buffer(&output, 0, &destination, 0, 4 * WORDS);
    queue.submit([encoder.finish()]);
    let read = read_back(&device, &destination);
    let words: Vec<u32> = read
        .chunks_exact(4)
        .map(|word| u32::from_le_bytes(word.try_into().unwrap()))
        .collect();
    assert_eq!(words, [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 0, 0, 0, 0]);
}
