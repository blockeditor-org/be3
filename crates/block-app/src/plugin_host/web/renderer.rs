use std::collections::HashMap;

use super::super::presenter::{BlitPipeline, SurfacePresenter};

pub(crate) fn presenter(
    device: &wgpu::Device,
    _queue: &wgpu::Queue,
) -> Result<WebSurfacePresenter, String> {
    Ok(WebSurfacePresenter {
        targets: HashMap::new(),
        premultiplied: device.adapter_info().backend == wgpu::Backend::BrowserWebGpu,
    })
}

pub(crate) struct WebFrame {
    pub(crate) size: [u32; 2],
    pub(crate) picture: Option<(u64, web_sys::ImageBitmap)>,
}

struct Target {
    size: [u32; 2],
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
    copied: Option<u64>,
}

pub(crate) struct WebSurfacePresenter {
    targets: HashMap<u32, Target>,
    premultiplied: bool,
}

impl WebSurfacePresenter {
    fn ensure_target(
        &mut self,
        device: &wgpu::Device,
        pipeline: &BlitPipeline,
        surface: u32,
        size: [u32; 2],
    ) {
        if self
            .targets
            .get(&surface)
            .is_some_and(|target| target.size == size)
        {
            return;
        }
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("plugin demo copy target"),
            size: wgpu::Extent3d {
                width: size[0],
                height: size[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: pipeline.copy_format(),
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = pipeline.texture_group(device, &view);
        self.targets.insert(
            surface,
            Target {
                size,
                texture,
                bind_group,
                copied: None,
            },
        );
    }

    fn copy_picture(&mut self, queue: &wgpu::Queue, surface: u32, frame: &WebFrame) {
        let Some(target) = self.targets.get_mut(&surface) else {
            return;
        };
        let Some((drawn, picture)) = &frame.picture else {
            return;
        };
        if target.copied == Some(*drawn) {
            return;
        }
        target.copied = Some(*drawn);
        let copy_size = wgpu::Extent3d {
            width: target.size[0].min(picture.width()),
            height: target.size[1].min(picture.height()),
            depth_or_array_layers: 1,
        };
        if copy_size.width == 0 || copy_size.height == 0 {
            return;
        }
        queue.copy_external_image_to_texture(
            &wgpu::CopyExternalImageSourceInfo {
                source: wgpu::ExternalImageSource::ImageBitmap(picture.clone()),
                origin: wgpu::Origin2d::ZERO,
                flip_y: false,
            },
            wgpu::CopyExternalImageDestInfo {
                texture: &target.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
                color_space: wgpu::PredefinedColorSpace::Srgb,
                premultiplied_alpha: self.premultiplied,
            },
            copy_size,
        );
    }
}

impl SurfacePresenter for WebSurfacePresenter {
    type Frame = WebFrame;

    fn replace(
        &mut self,
        device: &wgpu::Device,
        pipeline: &BlitPipeline,
        surface: u32,
        frame: &Self::Frame,
    ) -> Result<(), String> {
        if frame.size[0] > 0 && frame.size[1] > 0 {
            self.ensure_target(device, pipeline, surface, frame.size);
        }
        Ok(())
    }

    fn prepare(
        &mut self,
        queue: &wgpu::Queue,
        surface: u32,
        frame: &Self::Frame,
    ) -> Result<(), String> {
        if frame.size[0] > 0 && frame.size[1] > 0 {
            self.copy_picture(queue, surface, frame);
        }
        Ok(())
    }

    fn texture(&self, surface: u32) -> Option<&wgpu::BindGroup> {
        self.targets.get(&surface).map(|target| &target.bind_group)
    }

    fn release(&mut self, surface: u32) {
        self.targets.remove(&surface);
    }
}
