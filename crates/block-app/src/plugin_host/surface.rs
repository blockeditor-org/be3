use std::{cell::RefCell, collections::HashMap};

use crate::plugin_host::presenter::{BlitPipeline, SurfacePresenter};

thread_local! {
    static GPU: RefCell<Option<(wgpu::Device, wgpu::Queue)>> = const { RefCell::new(None) };
}

pub(super) fn gpu() -> Option<(wgpu::Device, wgpu::Queue)> {
    GPU.with(|gpu| gpu.borrow().clone())
}

pub(crate) struct SurfaceFrame {
    pub(crate) texture: wgpu::Texture,
    pub(crate) generation: u64,
}

struct Target {
    generation: u64,
    bind_group: wgpu::BindGroup,
}

pub(crate) struct Presenter {
    targets: HashMap<u32, Target>,
}

pub(crate) fn presenter(device: &wgpu::Device, queue: &wgpu::Queue) -> Result<Presenter, String> {
    GPU.with(|gpu| {
        *gpu.borrow_mut() = Some((device.clone(), queue.clone()));
    });
    Ok(Presenter {
        targets: HashMap::new(),
    })
}

impl SurfacePresenter for Presenter {
    type Frame = SurfaceFrame;

    fn replace(
        &mut self,
        device: &wgpu::Device,
        pipeline: &BlitPipeline,
        surface: u32,
        frame: &Self::Frame,
    ) -> Result<(), String> {
        if self
            .targets
            .get(&surface)
            .is_some_and(|target| target.generation == frame.generation)
        {
            return Ok(());
        }
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        self.targets.insert(
            surface,
            Target {
                generation: frame.generation,
                bind_group: pipeline.texture_group(device, &view),
            },
        );
        Ok(())
    }

    fn prepare(
        &mut self,
        _queue: &wgpu::Queue,
        _surface: u32,
        _frame: &Self::Frame,
    ) -> Result<(), String> {
        Ok(())
    }

    fn texture(&self, surface: u32) -> Option<&wgpu::BindGroup> {
        self.targets.get(&surface).map(|target| &target.bind_group)
    }

    fn release(&mut self, surface: u32) {
        self.targets.remove(&surface);
    }
}
