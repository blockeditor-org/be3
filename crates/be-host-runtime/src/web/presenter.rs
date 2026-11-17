use beui::native::texture_quad::{Bound, Hole, Pipelines, Quad};

use crate::presenter::SurfacePresenter;

pub struct Presenter {
    pipelines: Pipelines,
    hole: Bound,
}

pub fn presenter(device: &wgpu::Device, queue: &wgpu::Queue) -> Result<Presenter, String> {
    let Hole { pipelines, white } = Hole::new(device, queue);
    Ok(Presenter {
        hole: white,
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
