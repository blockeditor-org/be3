use stable_hash::HashMap;
use std::cell::RefCell;

use block_plugin_api::ScreenDamage;

use crate::backend::ShownFrame;
use crate::presenter::SurfacePresenter;

thread_local! {
    static GPU: RefCell<Option<(wgpu::Device, wgpu::Queue)>> = const { RefCell::new(None) };
}

pub(super) fn gpu() -> Option<(wgpu::Device, wgpu::Queue)> {
    GPU.with(|gpu| gpu.borrow().clone())
}

pub(super) fn copied(texture: &wgpu::Texture) -> Option<wgpu::Texture> {
    let (device, queue) = gpu()?;
    let copy = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("plugin preview"),
        size: texture.size(),
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: texture.format(),
        usage: wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_DST
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("plugin preview"),
    });
    encoder.copy_texture_to_texture(
        texture.as_image_copy(),
        copy.as_image_copy(),
        texture.size(),
    );
    queue.submit([encoder.finish()]);
    Some(copy)
}

pub(super) fn stop() {
    let gpu = GPU.with(|gpu| gpu.borrow_mut().take());
    drop(gpu);
}

pub struct SurfaceFrame {
    pub textures: Vec<PresentedTexture>,
    pub presents: u64,
    pub damage: Option<Vec<ScreenDamage>>,
}

#[derive(Clone)]
pub struct PresentedTexture {
    pub surface: u32,
    pub texture: wgpu::Texture,
    pub generation: u64,
}

impl ShownFrame for SurfaceFrame {
    fn presents(&self) -> u64 {
        self.presents
    }

    fn damage(&self) -> Option<&[ScreenDamage]> {
        self.damage.as_deref()
    }

    fn set_damage(&mut self, damage: Option<Vec<ScreenDamage>>) {
        self.damage = damage;
    }
}

use beui::native::texture_quad::{Bound, Pipelines};

struct Target {
    generation: u64,
    bound: Bound,
    size: (u32, u32),
}

pub struct Presenter {
    pipelines: Pipelines,
    targets: HashMap<(u32, u32), [Option<Target>; 2]>,
}

pub fn presenter(device: &wgpu::Device, queue: &wgpu::Queue) -> Result<Presenter, String> {
    GPU.with(|gpu| {
        *gpu.borrow_mut() = Some((device.clone(), queue.clone()));
    });
    Ok(Presenter {
        pipelines: Pipelines::new(device),
        targets: HashMap::default(),
    })
}

impl SurfacePresenter for Presenter {
    type Frame = SurfaceFrame;

    fn replace(
        &mut self,
        _device: &wgpu::Device,
        slot: u32,
        frame: &Self::Frame,
    ) -> Result<(), String> {
        for presented in &frame.textures {
            let [shown, other] = self.targets.entry((slot, presented.surface)).or_default();
            if shown
                .as_ref()
                .is_some_and(|target| target.generation == presented.generation)
            {
                continue;
            }
            std::mem::swap(shown, other);
            if shown
                .as_ref()
                .is_some_and(|target| target.generation == presented.generation)
            {
                continue;
            }
            let size = presented.texture.size();
            *shown = Some(Target {
                generation: presented.generation,
                bound: self.pipelines.bind(&presented.texture),
                size: (size.width, size.height),
            });
        }
        Ok(())
    }

    fn pipelines(&self) -> &Pipelines {
        &self.pipelines
    }

    fn texture(&self, slot: u32, surface: u32) -> Option<&Bound> {
        self.held(slot, surface).map(|target| &target.bound)
    }

    fn size(&self, slot: u32, surface: u32) -> Option<(u32, u32)> {
        self.held(slot, surface).map(|target| target.size)
    }

    fn retain(&mut self, slot: u32, surfaces: &[u32]) {
        self.targets
            .retain(|(held, surface), _| *held != slot || surfaces.contains(surface));
    }

    fn release(&mut self, slot: u32) {
        self.targets.retain(|(held, _), _| *held != slot);
    }
}

impl Presenter {
    fn held(&self, slot: u32, surface: u32) -> Option<&Target> {
        self.targets
            .get(&(slot, surface))
            .and_then(|[shown, _]| shown.as_ref())
    }
}
