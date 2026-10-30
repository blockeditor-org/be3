use super::*;

const ROW: u32 = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;

fn texture(device: &wgpu::Device) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: 4,
            height: 4,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    })
}

#[test]
fn texels_copied_through_two_textures_read_back_unchanged() {
    let (device, queue) = gpu();
    let size = wgpu::Extent3d {
        width: 4,
        height: 4,
        depth_or_array_layers: 1,
    };
    let texels: Vec<u8> = (0..(ROW * 4)).map(|byte| (byte % 251) as u8).collect();
    let source = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("texels"),
        size: u64::from(ROW * 4),
        usage: wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&source, 0, &texels);
    let (first, second) = (texture(&device), texture(&device));
    let destination = readable(&device, u64::from(ROW * 4));
    let layout = wgpu::TexelCopyBufferLayout {
        offset: 0,
        bytes_per_row: Some(ROW),
        rows_per_image: Some(4),
    };
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    encoder.copy_buffer_to_texture(
        wgpu::TexelCopyBufferInfo {
            buffer: &source,
            layout,
        },
        first.as_image_copy(),
        size,
    );
    encoder.copy_texture_to_texture(first.as_image_copy(), second.as_image_copy(), size);
    encoder.copy_texture_to_buffer(
        second.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &destination,
            layout,
        },
        size,
    );
    queue.submit([encoder.finish()]);
    let read = read_back(&device, &destination);
    for row in 0..4 {
        let start = (row * ROW) as usize;
        assert_eq!(
            read[start..start + 16],
            texels[start..start + 16],
            "row {row} of the texture"
        );
    }
}
