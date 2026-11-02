use beui::native::texture_quad::{Bound, Pipelines, Quad};

use crate::presenter::SurfacePresenter;

pub struct Presenter {
    pipelines: Pipelines,
    hole: Bound,
}

pub fn presenter(device: &wgpu::Device, queue: &wgpu::Queue) -> Result<Presenter, String> {
    let size = wgpu::Extent3d {
        width: 1,
        height: 1,
        depth_or_array_layers: 1,
    };
    let hole = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("plugin canvas hole"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        hole.as_image_copy(),
        &[255; 4],
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4),
            rows_per_image: Some(1),
        },
        size,
    );
    let pipelines = Pipelines::erasing(device);
    Ok(Presenter {
        hole: pipelines.bind(&hole),
        pipelines,
    })
}

impl SurfacePresenter for Presenter {
    type Frame = super::Frame;

    fn replace(
        &mut self,
        _device: &wgpu::Device,
        _slot: u32,
        _frame: &super::Frame,
    ) -> Result<(), String> {
        Ok(())
    }

    fn pipelines(&self) -> &Pipelines {
        &self.pipelines
    }

    fn texture(&self, _slot: u32, _surface: u32) -> Option<&Bound> {
        Some(&self.hole)
    }

    fn shown(&self, quad: Quad) -> Quad {
        Quad {
            opacity: 1.0,
            ..quad
        }
    }

    fn retain(&mut self, _slot: u32, _surfaces: &[u32]) {}

    fn release(&mut self, _slot: u32) {}
}
