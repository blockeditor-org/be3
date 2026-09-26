use super::*;

fn texture(gpu: &mut Gpu, usage: wgpu::TextureUsages) -> abi::Handle {
    let texture = gpu.device().create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: 8,
            height: 8,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage,
        view_formats: &[],
    });
    gpu.textures.insert(texture)
}

fn whole(texture: abi::Handle) -> abi::TexelCopyTextureInfo {
    abi::TexelCopyTextureInfo {
        texture,
        mip_level: 0,
        origin_x: 0,
        origin_y: 0,
        origin_z: 0,
        aspect: abi::TextureAspect::All,
    }
}

#[test]
fn a_texture_copy_is_recorded_on_its_encoder() {
    let mut gpu = gpu();
    let source = texture(&mut gpu, wgpu::TextureUsages::COPY_SRC);
    let destination = texture(&mut gpu, wgpu::TextureUsages::COPY_DST);
    let encoder = gpu.create_command_encoder(&abi::encode(&abi::CommandEncoderDescriptor {
        label: "copy".into(),
    }));
    let copy = |encoder| {
        abi::encode(&abi::CopyTextureToTexture {
            encoder,
            source: whole(source),
            destination: whole(destination),
            size: abi::Extent3d {
                width: 8,
                height: 8,
                depth_or_array_layers: 1,
            },
        })
    };
    gpu.copy_texture_to_texture(&copy(encoder));
    let buffer = gpu.finish_encoder(encoder);
    gpu.submit(&[buffer]);
    assert_eq!(gpu.take_error(), None);
    gpu.copy_texture_to_texture(&copy(encoder));
    let error = gpu
        .take_error()
        .expect("a copy on a finished encoder should report");
    assert!(error.contains("command encoder"), "{error}");
}
