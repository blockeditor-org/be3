use super::*;

const SHADER: &str = "
override reach: f32 = 0.0;
@id(1) override green: f32 = 0.0;

@vertex
fn vertex(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let corners = array(vec2(-1.0, -1.0), vec2(reach, -1.0), vec2(-1.0, reach));
    return vec4(corners[index], 0.0, 1.0);
}

@fragment
fn fragment() -> @location(0) vec4<f32> {
    return vec4(0.0, green, 0.0, 1.0);
}
";

const ROW: u32 = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;

#[test]
fn a_render_pipeline_draws_with_the_values_given_to_its_overridable_constants() {
    let (device, queue) = gpu();
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl(SHADER.into()),
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: None,
        layout: None,
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: None,
            compilation_options: wgpu::PipelineCompilationOptions {
                constants: &[("reach", 3.0)],
                ..Default::default()
            },
            buffers: &[],
        },
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: None,
            compilation_options: wgpu::PipelineCompilationOptions {
                constants: &[("1", 1.0)],
                ..Default::default()
            },
            targets: &[Some(format.into())],
        }),
        multiview_mask: None,
        cache: None,
    });
    let size = wgpu::Extent3d {
        width: 4,
        height: 4,
        depth_or_array_layers: 1,
    };
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = target.create_view(&wgpu::TextureViewDescriptor::default());
    let destination = readable(&device, u64::from(ROW * 4));
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
        pass.set_pipeline(&pipeline);
        pass.draw(0..3, 0..1);
    }
    encoder.copy_texture_to_buffer(
        target.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &destination,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(ROW),
                rows_per_image: None,
            },
        },
        size,
    );
    queue.submit([encoder.finish()]);
    let read = read_back(&device, &destination);
    for row in 0..4 {
        let start = (row * ROW) as usize;
        assert_eq!(
            read[start..start + 16],
            [0, 255, 0, 255].repeat(4),
            "row {row} is covered by the triangle the vertex constant stretched, in the fragment constant's green"
        );
    }
}
