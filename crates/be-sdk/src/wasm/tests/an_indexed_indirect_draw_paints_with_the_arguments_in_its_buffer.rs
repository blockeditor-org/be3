use wgpu::util::DeviceExt;

use super::*;

#[test]
fn an_indexed_indirect_draw_paints_with_the_arguments_in_its_buffer() {
    let (device, queue) = gpu();
    let pipeline = filling(&device);
    let indices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("indices"),
        contents: &[0u32, 1, 2]
            .iter()
            .flat_map(|index| index.to_le_bytes())
            .collect::<Vec<u8>>(),
        usage: wgpu::BufferUsages::INDEX,
    });
    let arguments = wgpu::util::DrawIndexedIndirectArgs {
        index_count: 3,
        instance_count: 1,
        first_index: 0,
        base_vertex: 0,
        first_instance: 0,
    };
    let indirect = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("arguments"),
        contents: arguments.as_bytes(),
        usage: wgpu::BufferUsages::INDIRECT,
    });
    let target = filled_target(&device);
    let view = target.create_view(&wgpu::TextureViewDescriptor::default());
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            color_attachments: &[cleared(&view)],
            ..Default::default()
        });
        pass.set_pipeline(&pipeline);
        pass.set_index_buffer(indices.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed_indirect(&indirect, 0);
    }
    assert_eq!(
        texels(&device, &queue, encoder, &target),
        vec![MAGENTA; 16],
        "the draw read three indices from its buffer"
    );
}
