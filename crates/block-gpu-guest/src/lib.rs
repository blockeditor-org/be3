mod convert;
mod features;
mod objects;

#[cfg(test)]
mod tests;

use std::{cell::RefCell, collections::HashMap};

use block_gpu_abi::{device as abi, guest::device as gpu};

thread_local! {
    static SURFACES: RefCell<HashMap<u32, gpu::Surface>> = RefCell::new(HashMap::new());
}

pub fn features() -> wgpu::Features {
    features::features(&gpu::features())
}

pub fn limits() -> wgpu::Limits {
    features::limits(&gpu::limits())
}

pub fn request_device(
    descriptor: &wgpu::DeviceDescriptor<'_>,
) -> Result<(wgpu::Device, wgpu::Queue), String> {
    let offered = features();
    features::requested(descriptor, offered, &limits())?;
    let device = objects::Device::new(
        descriptor.required_features,
        offered,
        descriptor.required_limits.clone(),
    );
    Ok((
        wgpu::Device::from_custom(device),
        wgpu::Queue::from_custom(objects::Queue),
    ))
}

fn with_surface<R>(surface: u32, act: impl FnOnce(&gpu::Surface) -> R) -> R {
    SURFACES.with(|surfaces| {
        let mut surfaces = surfaces.borrow_mut();
        act(surfaces
            .entry(surface)
            .or_insert_with(|| gpu::Surface::new(surface)))
    })
}

pub fn configure_surface(surface: u32, width: u32, height: u32, format: wgpu::TextureFormat) {
    with_surface(surface, |surface| {
        surface.configure(abi::SurfaceConfiguration {
            width,
            height,
            format: convert::texture_format(format),
        })
    });
}

pub fn acquire_surface_texture(surface: u32) -> Result<wgpu::Texture, String> {
    let Some(texture) = with_surface(surface, gpu::Surface::acquire) else {
        return Err(objects::last_error());
    };
    described(texture)
}

pub fn import_texture(image: u64) -> Option<wgpu::Texture> {
    described(gpu::import_texture(image)?).ok()
}

fn described(texture: gpu::Texture) -> Result<wgpu::Texture, String> {
    let Some(info) = texture.describe() else {
        return Err(objects::last_error());
    };
    let descriptor = wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: info.size.width,
            height: info.size.height,
            depth_or_array_layers: info.size.depth_or_array_layers,
        },
        mip_level_count: info.mip_level_count,
        sample_count: info.sample_count,
        dimension: convert::wgpu_texture_dimension(info.dimension),
        format: convert::wgpu_texture_format(info.format),
        usage: wgpu::TextureUsages::from_bits_truncate(info.usage),
        view_formats: &[],
    };
    Ok(wgpu::Texture::from_custom(
        objects::Texture(texture),
        &descriptor,
    ))
}

pub fn surface_age(surface: u32) -> u32 {
    with_surface(surface, gpu::Surface::age)
}

pub fn present_surface(surface: u32) {
    with_surface(surface, gpu::Surface::present);
}

pub fn release_surface(surface: u32) {
    let released = SURFACES.with(|surfaces| surfaces.borrow_mut().remove(&surface));
    drop(released.unwrap_or_else(|| gpu::Surface::new(surface)));
}
