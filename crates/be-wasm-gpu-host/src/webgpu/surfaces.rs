//! The surfaces the host shows an app, as wasi:webgpu canvas contexts (`be3:gpu/host`): the
//! app chooses their size with `resize`, as a page sets a canvas's width and height, and
//! their format and usage with `configure`.

use be_wasm_gpu_abi::embed::host::Host;
use be_wasm_gpu_abi::webgpu::webgpu as web;
use be_wasm_gpu_convert::Convert;

use super::{Canvas, Failure, Kind};
use crate::{Gpu, SURFACE_USAGE};

impl Gpu {
    fn context_surface(&self, context: u32) -> Option<u32> {
        self.web
            .contexts
            .get(context, "canvas context")
            .ok()
            .map(|context| context.surface)
    }

    /// Gives a context's surface textures once it has a size and a configuration, unless it
    /// has them already; a frame in progress ends unpresented.
    fn remake(&mut self, context: u32) {
        let Ok(state) = self.web.contexts.get_mut(context, "canvas context") else {
            return;
        };
        state.current = None;
        let (surface, size, configuration) =
            (state.surface, state.size, state.configuration.clone());
        let (Some((width, height)), Some(configuration)) = (size, configuration) else {
            return;
        };
        let device = configuration.device.0;
        let format: wgpu::TextureFormat = configuration.format.convert();
        let limit = self.device_has(device).map_or(
            self.device.limits().max_texture_dimension_2d,
            |(_, limits)| limits.max_texture_dimension_2d,
        );
        if width == 0 || height == 0 || width > limit || height > limit {
            self.detach_surface(surface);
            self.raise(
                Some(device),
                Failure::validation(format!(
                    "a {width} by {height} canvas is more than the device can make, or empty"
                )),
            );
            return;
        }
        let mut view_formats: Vec<wgpu::TextureFormat> = Vec::new();
        for view in configuration.view_formats.iter().flatten() {
            let view: wgpu::TextureFormat = view.convert();
            if view != format && !view_formats.contains(&view) {
                view_formats.push(view);
            }
        }
        let usage = SURFACE_USAGE
            | configuration
                .usage
                .map_or(wgpu::TextureUsages::RENDER_ATTACHMENT, Convert::convert);
        let look = crate::looks::Look {
            opaque: configuration.alpha_mode != Some(web::GpuCanvasAlphaMode::Premultiplied),
            display_p3: configuration.color_space == Some(web::PredefinedColorSpace::DisplayP3),
            clamped: format == wgpu::TextureFormat::Rgba16Float,
        };
        let matches = self.surfaces.get(&surface).is_some_and(|existing| {
            let (texture, _) = &existing.textures[0];
            existing.textures.len() == 2
                && existing.view_formats == view_formats
                && texture.width() == width
                && texture.height() == height
                && texture.format() == format
                && texture.usage() == usage
        });
        if matches {
            return self.look(surface, look);
        }
        let made = self.wrapped(Some(device), |gpu| {
            let texture = || {
                gpu.device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("app canvas"),
                    size: wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage,
                    view_formats: &view_formats,
                })
            };
            // A device that cannot make this canvas (an sRGB view on WebGL2, which has no
            // `VIEW_FORMATS`) refuses the configuration: the canvas keeps no textures, rather
            // than invalid ones the compositor would trip over.
            let scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
            let textures = vec![texture(), texture()];
            match crate::caught(scope.pop()) {
                Some(error) => Err(Failure::validation(format!(
                    "the device cannot make a {format:?} canvas viewed as {view_formats:?}: {error}"
                ))),
                None => Ok(textures),
            }
        });
        match made {
            Some(textures) => {
                self.insert_surface(surface, textures, view_formats);
                self.look(surface, look);
            }
            None => self.detach_surface(surface),
        }
    }

    pub(super) fn configure_context(
        &mut self,
        context: u32,
        configuration: web::GpuCanvasConfiguration,
    ) {
        let device = Some(configuration.device.0);
        let format: wgpu::TextureFormat = configuration.format.convert();
        let canvas = [
            wgpu::TextureFormat::Bgra8Unorm,
            wgpu::TextureFormat::Rgba8Unorm,
            wgpu::TextureFormat::Rgba16Float,
        ];
        if !canvas.contains(&format) {
            return self.raise(
                device,
                Failure::validation(format!(
                    "a canvas is bgra8unorm, rgba8unorm or rgba16float, not {format:?}"
                )),
            );
        }
        if let Some(view) = configuration
            .view_formats
            .iter()
            .flatten()
            .map(|view| Convert::<wgpu::TextureFormat>::convert(*view))
            .find(|view| view.remove_srgb_suffix() != format)
        {
            return self.raise(
                device,
                Failure::validation(format!(
                    "a {format:?} canvas may be viewed as its format with or without the sRGB suffix, not {view:?}"
                )),
            );
        }
        let Ok(state) = self.web.contexts.get_mut(context, "canvas context") else {
            return;
        };
        state.configuration = Some(web::GpuCanvasConfigurationOwned {
            device: configuration.device,
            format: configuration.format,
            usage: configuration.usage,
            view_formats: configuration.view_formats,
            color_space: configuration.color_space,
            tone_mapping: configuration.tone_mapping,
            alpha_mode: configuration.alpha_mode,
        });
        self.remake(context);
    }

    pub(super) fn unconfigure_context(&mut self, context: u32) {
        let Ok(state) = self.web.contexts.get_mut(context, "canvas context") else {
            return;
        };
        state.configuration = None;
        state.current = None;
        let surface = state.surface;
        self.detach_surface(surface);
    }

    /// The texture of the frame in progress, as a fresh handle each time it is asked for: the
    /// same texture until `present`. An invalid texture when the context has none, or its
    /// device is lost.
    pub(super) fn current_texture(&mut self, context: u32) -> u32 {
        let Ok(state) = self.web.contexts.get(context, "canvas context") else {
            return self.textures.skip();
        };
        let device = state
            .configuration
            .as_ref()
            .map(|configuration| configuration.device.0);
        let surface = state.surface;
        let current = state.current.clone();
        let Some(device) = device else {
            self.raise(
                None,
                Failure::validation("an unconfigured canvas has no texture"),
            );
            return self.textures.skip();
        };
        if self.device_lost(device) {
            return self.textures.skip();
        }
        let texture = match current {
            Some(texture) => texture,
            None => {
                let Some(target) = self.surfaces.get(&surface) else {
                    self.raise(
                        Some(device),
                        Failure::validation("the canvas has no size: the app has not resized it"),
                    );
                    return self.textures.skip();
                };
                let texture = target.textures[target.drawn].0.clone();
                if let Ok(state) = self.web.contexts.get_mut(context, "canvas context") {
                    state.current = Some(texture.clone());
                }
                texture
            }
        };
        let handle = self.textures.insert(texture);
        self.own(Kind::Texture, handle, Some(device), None);
        handle
    }

    pub(super) fn drop_context(&mut self, context: u32) {
        if let Ok(state) = self.web.contexts.take(context, "canvas context") {
            self.detach_surface(state.surface);
        }
    }
}

impl Host for Gpu {
    fn surface(&mut self, id: u32) -> web::GpuCanvasContext {
        web::GpuCanvasContext(self.web.contexts.insert(Canvas {
            surface: id,
            configuration: None,
            size: None,
            current: None,
        }))
    }

    fn resize(&mut self, context: web::GpuCanvasContext, width: u32, height: u32) {
        let Ok(state) = self.web.contexts.get_mut(context.0, "canvas context") else {
            return;
        };
        if state.size == Some((width, height)) {
            return;
        }
        state.size = Some((width, height));
        self.remake(context.0);
    }

    fn present(&mut self, context: web::GpuCanvasContext) {
        let Some(surface) = self.context_surface(context.0) else {
            return;
        };
        if let Ok(state) = self.web.contexts.get_mut(context.0, "canvas context") {
            state.current = None;
        }
        if let Some(target) = self.surfaces.get_mut(&surface) {
            target.drawn = (target.drawn + 1) % target.textures.len();
            target.ages.present();
        }
        if let Some(target) = self.surfaces.get(&surface) {
            if !target.shown.is_empty() && !self.looking.contains(&surface) {
                self.looking.push(surface);
            }
            self.last_presented.insert(surface, target.shown().clone());
        }
        self.presented.push(surface);
    }

    fn age(&mut self, context: web::GpuCanvasContext) -> u32 {
        self.context_surface(context.0)
            .and_then(|surface| self.surfaces.get(&surface))
            .map_or(0, |target| target.ages.age())
    }

    fn import_texture(&mut self, image: u64) -> Option<web::GpuTexture> {
        let texture = self
            .imports
            .lock()
            .unwrap_or_else(|held| held.into_inner())
            .get(&image)
            .cloned();
        match texture {
            Some(texture) => {
                let handle = self.textures.insert(texture);
                self.web.imported.insert(handle);
                // `import-texture` names no device: the texture is the app's newest device's,
                // whose scopes hear what its use gets wrong.
                let device = self
                    .web
                    .devices
                    .handles()
                    .filter(|device| !self.device_lost(*device))
                    .last();
                self.own(Kind::Texture, handle, device, None);
                Some(web::GpuTexture(handle))
            }
            None => {
                self.textures.skip();
                None
            }
        }
    }
}
