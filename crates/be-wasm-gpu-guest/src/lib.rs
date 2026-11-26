//! wgpu's custom backend over wasi:webgpu's guest bindings (decisions/be3_gpu_is_wasi_webgpu.md):
//! an app's wgpu calls, answered by the host on the app's own thread. Surfaces and shared
//! images are `be3:gpu/host`'s.
//!
//! The API is what browser WebGPU exposes, since the same app runs in a browser; anything
//! missing is an `unimplemented!` in `objects.rs`. Whatever wgpu marks `unsafe`
//! (passthrough shaders, pipeline caches) is refused for good: apps are untrusted.

mod convert;
mod features;
mod objects;
mod passes;

#[cfg(test)]
mod tests;

use std::{cell::RefCell, collections::HashMap};

use be_wasm_gpu_abi::guest::{host, webgpu as gpu};
use be_wasm_gpu_abi::webgpu::webgpu as web;
use be_wasm_gpu_convert::Convert;
use be_wasm_ops::local::Ticket;

thread_local! {
    static SURFACES: RefCell<HashMap<u32, gpu::GpuCanvasContext>> = RefCell::new(HashMap::new());
    static ADAPTER: RefCell<Option<gpu::GpuAdapter>> = const { RefCell::new(None) };
}

/// Polls until `ticket` settles. The host answers an async call at once and settles it at the
/// next poll, so this waits one round.
pub(crate) fn settled<T>(mut ticket: Ticket<T>) -> T {
    loop {
        gpu::settle(true);
        if let Some(value) = ticket.take() {
            return value;
        }
    }
}

/// The host's adapter, asked for once.
fn with_adapter<R>(act: impl FnOnce(&gpu::GpuAdapter) -> R) -> R {
    ADAPTER.with(|adapter| {
        let mut adapter = adapter.borrow_mut();
        let adapter = adapter.get_or_insert_with(|| {
            let found =
                settled(gpu::get_gpu().request_adapter(&None)).expect("the host offers an adapter");
            gpu::GpuAdapter::from_handle(found)
        });
        act(adapter)
    })
}

pub fn features() -> wgpu::Features {
    with_adapter(|adapter| features::features(&adapter.features()))
}

pub fn limits() -> wgpu::Limits {
    with_adapter(|adapter| features::limits(&adapter.limits()))
}

/// The app's device, with only the features and limits it asks for: the host checks the app's
/// calls against those, not against what the host's device has. `indirect-first-instance`
/// cannot be checked (the first instance is in a gpu buffer), so an app that did not request it
/// gets whatever the host's device does.
///
/// Calling it again after `Device::destroy` is how an app gets a working device back.
pub fn request_device(
    descriptor: &wgpu::DeviceDescriptor<'_>,
) -> Result<(wgpu::Device, wgpu::Queue), String> {
    let offered = features();
    features::requested(descriptor, offered, &limits())?;
    let asked = features::descriptor(descriptor);
    let made = with_adapter(|adapter| settled(adapter.request_device(&Some(asked.descriptor))));
    drop(asked.limits);
    let device = made.map_err(|error| error.message)?;
    be_wasm_gpu_abi::set_app_device(device);
    let device = gpu::GpuDevice::from_handle(device);
    let queue = device.queue();
    let device = objects::Device::new(
        device,
        descriptor.required_features,
        descriptor.required_limits.clone(),
    );
    Ok((
        wgpu::Device::from_custom(device),
        wgpu::Queue::from_custom(objects::Queue(queue)),
    ))
}

/// `desc`'s pipeline, compiled by the host without holding up the app's thread where it can, as
/// WebGPU's `createRenderPipelineAsync`, which wgpu has no call for. A pipeline that fails comes
/// back as an error rather than as a validation error.
pub async fn create_render_pipeline_async(
    device: &wgpu::Device,
    desc: &wgpu::RenderPipelineDescriptor<'_>,
) -> Result<wgpu::RenderPipeline, String> {
    objects::render_pipeline_async(device, desc).await?;
    Ok(device.create_render_pipeline(desc))
}

/// `desc`'s pipeline, as `create_render_pipeline_async` makes one.
pub async fn create_compute_pipeline_async(
    device: &wgpu::Device,
    desc: &wgpu::ComputePipelineDescriptor<'_>,
) -> Result<wgpu::ComputePipeline, String> {
    objects::compute_pipeline_async(device, desc).await?;
    Ok(device.create_compute_pipeline(desc))
}

/// Runs the callbacks of maps, `on_submitted_work_done`, compilation info, error scopes,
/// uncaptured errors and device loss that settled since the last poll. The host only wakes the
/// app; nothing is called back until the guest polls, which be-sdk does at the start of every
/// step.
pub fn poll() {
    objects::settle(false);
}

fn with_surface<R>(surface: u32, act: impl FnOnce(&gpu::GpuCanvasContext) -> R) -> R {
    SURFACES.with(|surfaces| {
        let mut surfaces = surfaces.borrow_mut();
        act(surfaces
            .entry(surface)
            .or_insert_with(|| host::surface(surface)))
    })
}

/// Gives `surface` textures of this size and `format` (a canvas format, never sRGB), which may
/// also be viewed as each of `view_formats` (only its sRGB twin; a host on WebGL can view a
/// texture in no other format), configured with the app's device. `alpha` is how the host
/// takes its alpha: `Opaque` ignores it, `PreMultiplied` blends it as premultiplied, as a
/// translucent surface needs; `Auto` is WebGPU's default, opaque.
pub fn configure_surface(
    surface: u32,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
    view_formats: &[wgpu::TextureFormat],
    alpha: wgpu::CompositeAlphaMode,
) {
    let alpha_mode = match alpha {
        wgpu::CompositeAlphaMode::Opaque => Some(web::GpuCanvasAlphaMode::Opaque),
        wgpu::CompositeAlphaMode::PreMultiplied => Some(web::GpuCanvasAlphaMode::Premultiplied),
        wgpu::CompositeAlphaMode::Auto => None,
        other => panic!("a canvas's alpha is opaque or premultiplied, not {other:?}"),
    };
    let Some(device) = be_wasm_gpu_abi::app_device() else {
        return;
    };
    with_surface(surface, |context| {
        host::resize(context, width, height);
        context.configure(&web::GpuCanvasConfiguration {
            device,
            format: convert::to_web(format),
            usage: None,
            view_formats: Some(
                view_formats
                    .iter()
                    .map(|view| convert::to_web(*view))
                    .collect(),
            ),
            color_space: None,
            tone_mapping: None,
            alpha_mode,
        });
    });
}

/// The texture of `surface`'s frame in progress, the same one until `present_surface`.
pub fn acquire_surface_texture(surface: u32) -> Result<wgpu::Texture, String> {
    let texture = with_surface(surface, gpu::GpuCanvasContext::get_current_texture);
    described(texture)
}

/// `texture`, from the guest bindings, as a wgpu texture to draw into.
pub fn texture(texture: gpu::GpuTexture) -> Result<wgpu::Texture, String> {
    described(texture)
}

pub fn import_texture(image: u64) -> Option<wgpu::Texture> {
    let texture = host::import_texture(image)?;
    described(texture).ok()
}

fn described(texture: gpu::GpuTexture) -> Result<wgpu::Texture, String> {
    let (width, height) = (texture.width(), texture.height());
    if width == 0 || height == 0 {
        return Err("the texture is invalid: the host could not make it".to_owned());
    }
    let format = texture.format();
    let descriptor = wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: texture.depth_or_array_layers(),
        },
        mip_level_count: texture.mip_level_count(),
        sample_count: texture.sample_count(),
        dimension: texture.dimension().convert(),
        format: format.convert(),
        usage: texture.usage().convert(),
        view_formats: &[],
    };
    Ok(wgpu::Texture::from_custom(
        objects::Texture(texture),
        &descriptor,
    ))
}

pub fn surface_age(surface: u32) -> u32 {
    with_surface(surface, |context| host::age(context))
}

pub fn present_surface(surface: u32) {
    with_surface(surface, |context| host::present(context));
}

/// Lets the host forget `surface`.
pub fn release_surface(surface: u32) {
    SURFACES.with(|surfaces| surfaces.borrow_mut().remove(&surface));
}
