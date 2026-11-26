use super::web::*;
use super::*;

/// A texture the host shares between apps is only sampled: whatever usage the host gave it,
/// an app's views of it, its usage, writes and copies into it, and binding it directly as a
/// storage texture are refused.
#[test]
fn an_imported_texture_is_only_read() {
    let mut gpu = gpu();
    let device = device(&mut gpu, Vec::new()).unwrap();
    let shared = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("shared"),
        size: wgpu::Extent3d {
            width: 4,
            height: 4,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::STORAGE_BINDING
            | wgpu::TextureUsages::COPY_DST
            | wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    gpu.imports().lock().unwrap().insert(9, shared);
    let texture = be_wasm_gpu_abi::embed::host::Host::import_texture(&mut gpu, 9)
        .expect("the image is shared");
    assert_eq!(
        gpu.gpu_texture_usage(GpuTexture(texture.0)),
        GpuTextureUsage::TEXTURE_BINDING
    );

    let refused = |gpu: &mut Gpu, ticket: u32, what: &str| {
        let caught = pop(gpu, device, ticket)
            .expect("the scope pops")
            .unwrap_or_else(|| panic!("{what} is refused"));
        gpu.gpu_error_message(caught)
    };

    gpu.gpu_device_push_error_scope(device, GpuErrorFilter::Validation);
    let _view = gpu.gpu_texture_create_view(
        GpuTexture(texture.0),
        Some(GpuTextureViewDescriptor {
            format: None,
            dimension: None,
            usage: Some(GpuTextureUsage::RENDER_ATTACHMENT),
            aspect: None,
            base_mip_level: None,
            mip_level_count: None,
            base_array_layer: None,
            array_layer_count: None,
            swizzle: None,
            label: None,
        }),
    );
    refused(&mut gpu, 1, "a view to render into");

    let destination = || GpuTexelCopyTextureInfo {
        texture: GpuTexture(texture.0),
        mip_level: None,
        origin: None,
        aspect: None,
    };
    let extent = || GpuExtent3D {
        width: 4,
        height: Some(4),
        depth_or_array_layers: None,
    };
    let queue = gpu.gpu_device_queue(device);
    gpu.gpu_device_push_error_scope(device, GpuErrorFilter::Validation);
    gpu.gpu_queue_write_texture_with_copy(
        queue,
        destination(),
        &[0; 64],
        GpuTexelCopyBufferLayout {
            offset: None,
            bytes_per_row: Some(16),
            rows_per_image: None,
        },
        extent(),
    );
    assert!(refused(&mut gpu, 2, "a write").contains("only read"));

    let source = gpu.gpu_device_create_buffer(device, buffer(GpuBufferUsage::COPY_SRC));
    let encoder = gpu.gpu_device_create_command_encoder(device, None);
    gpu.gpu_device_push_error_scope(device, GpuErrorFilter::Validation);
    gpu.gpu_command_encoder_copy_buffer_to_texture(
        encoder,
        GpuTexelCopyBufferInfo {
            buffer: GpuBuffer(source.0),
            offset: None,
            bytes_per_row: Some(256),
            rows_per_image: None,
        },
        destination(),
        extent(),
    );
    assert!(refused(&mut gpu, 3, "a copy").contains("only read"));

    let layout = gpu.gpu_device_create_bind_group_layout(
        device,
        GpuBindGroupLayoutDescriptor {
            entries: vec![GpuBindGroupLayoutEntry {
                binding: 0,
                visibility: GpuShaderStage::COMPUTE,
                buffer: None,
                sampler: None,
                texture: None,
                storage_texture: Some(GpuStorageTextureBindingLayout {
                    access: Some(GpuStorageTextureAccess::WriteOnly),
                    format: GpuTextureFormat::Rgba8unorm,
                    view_dimension: None,
                }),
            }],
            label: None,
        },
    );
    gpu.gpu_device_push_error_scope(device, GpuErrorFilter::Validation);
    let _group = gpu.gpu_device_create_bind_group(
        device,
        GpuBindGroupDescriptor {
            layout: GpuBindGroupLayout(layout.0),
            entries: vec![GpuBindGroupEntry {
                binding: 0,
                resource: GpuBindingResource::GpuTexture(GpuTexture(texture.0)),
            }],
            label: None,
        },
    );
    refused(&mut gpu, 4, "binding it to be written");
}
