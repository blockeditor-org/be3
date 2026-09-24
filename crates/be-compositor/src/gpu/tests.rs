use super::*;

mod a_format_the_compositor_cannot_read_is_refused;
mod a_linear_dmabuf_imports_with_its_pixels;
mod linear_argb_is_advertised_for_sampling;

use std::io::Write as _;

use smithay::backend::allocator::dmabuf::DmabufFlags;

pub(crate) fn vulkan_device() -> (wgpu::Device, wgpu::Queue) {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::VULKAN,
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        force_fallback_adapter: false,
        compatible_surface: None,
    }))
    .expect("a Vulkan adapter is available");
    open_device(
        &adapter,
        &wgpu::DeviceDescriptor {
            label: Some("compositor test device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::downlevel_defaults(),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::Off,
        },
    )
    .expect("the Vulkan device opens")
}

pub(crate) fn pattern(width: u32, height: u32) -> Vec<u8> {
    (0..width * height)
        .flat_map(|index| [(index % 251) as u8, (index / 251 % 251) as u8, 90, 255])
        .collect()
}

pub(crate) fn memfd_dmabuf(width: u32, height: u32, code: Fourcc, pixels: &[u8]) -> Dmabuf {
    let fd =
        rustix::fs::memfd_create("dmabuf", rustix::fs::MemfdFlags::CLOEXEC).expect("a memfd opens");
    let mut file = std::fs::File::from(fd);
    file.write_all(pixels).expect("the pixels are written");
    let mut builder = Dmabuf::builder(
        (width as i32, height as i32),
        code,
        Modifier::Linear,
        DmabufFlags::empty(),
    );
    builder.add_plane(file.into(), 0, 0, width * 4);
    builder.build().expect("the dmabuf builds")
}

pub(crate) fn read(device: &wgpu::Device, queue: &wgpu::Queue, texture: &wgpu::Texture) -> Vec<u8> {
    let size = texture.size();
    let row = size.width * 4;
    let padded = row.div_ceil(256) * 256;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: u64::from(padded * size.height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded),
                rows_per_image: Some(size.height),
            },
        },
        size,
    );
    queue.submit([encoder.finish()]);
    buffer.slice(..).map_async(wgpu::MapMode::Read, |result| {
        result.expect("the readback maps");
    });
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("the device finishes");
    let mapped = buffer.slice(..).get_mapped_range();
    mapped
        .chunks(padded as usize)
        .flat_map(|chunk| chunk[..row as usize].to_vec())
        .collect()
}
