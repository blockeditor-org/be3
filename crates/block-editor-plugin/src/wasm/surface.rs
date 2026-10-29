use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};

use block_plugin_api::{FrameCommit, PresentedFrame, ScreenLayout, SurfaceFormat, SurfaceSpec};

use crate::{
    panes::{Panes, ShownLayer},
    screens::Screens,
};

thread_local! {
    static GPU: RefCell<Option<Gpu>> = const { RefCell::new(None) };
    static FORMAT: Cell<Option<wgpu::TextureFormat>> = const { Cell::new(None) };
    static PRESENTS: Cell<u64> = const { Cell::new(0) };
}

#[derive(Clone)]
struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
}

fn gpu() -> Option<Gpu> {
    GPU.with(|gpu| {
        let mut gpu = gpu.borrow_mut();
        if gpu.is_none() && FORMAT.get().is_some() {
            let (device, queue) = block_gpu_guest::device_and_queue();
            *gpu = Some(Gpu { device, queue });
        }
        gpu.clone()
    })
}

pub(crate) fn surface_gpu() -> Option<crate::SurfaceGpu> {
    let Gpu { device, queue } = gpu()?;
    Some(crate::SurfaceGpu {
        device,
        queue,
        format: FORMAT.get()?,
    })
}

pub(crate) struct Surfaces {
    gpu: Option<(Gpu, wgpu::TextureFormat)>,
    panes: Panes,
    layout: ScreenLayout,
    fresh: HashSet<u32>,
    layers: HashMap<u32, [u32; 2]>,
}

impl Surfaces {
    pub(crate) fn new(spec: SurfaceSpec) -> Result<Self, String> {
        let gpu = match spec.render {
            true => {
                FORMAT.set(Some(format(spec.format)));
                Some((
                    gpu().ok_or("the plugin gpu is not ready")?,
                    format(spec.format),
                ))
            }
            false => None,
        };
        Ok(Self {
            gpu,
            panes: Panes::default(),
            layout: ScreenLayout::default(),
            fresh: HashSet::new(),
            layers: HashMap::new(),
        })
    }

    pub(crate) fn layout(&self) -> &ScreenLayout {
        &self.layout
    }

    pub(crate) fn place(&mut self, layout: ScreenLayout) {
        let Some((_, format)) = &self.gpu else {
            self.layout = layout;
            return;
        };
        let format = *format;
        for gone in &self.layout.screens {
            if !layout
                .screens
                .iter()
                .any(|placement| placement.surface == gone.surface)
            {
                block_gpu_guest::release_surface(gone.surface);
                self.fresh.remove(&gone.surface);
            }
        }
        for placement in &layout.screens {
            let kept = self.layout.screens.iter().any(|previous| {
                previous.surface == placement.surface
                    && (previous.width, previous.height) == (placement.width, placement.height)
            });
            if !kept {
                block_gpu_guest::configure_surface(
                    placement.surface,
                    placement.width,
                    placement.height,
                    format,
                );
                self.fresh.insert(placement.surface);
            }
        }
        self.layout = layout;
    }

    pub(crate) fn render(&mut self, screens: &mut Screens) -> Result<Option<FrameCommit>, String> {
        if self.layout.is_empty() {
            for (surface, _) in self.layers.drain() {
                block_gpu_guest::release_surface(surface);
            }
            return Ok(None);
        }
        let ran = self.panes.run(
            &self.layout,
            screens,
            &self.fresh,
            crate::screens::seen(),
            self.gpu.is_some(),
        );
        let repaint = ran.repaint;
        let animating = ran.animating;
        let layers = self.place_layers(screens);
        let seen = crate::screens::seen();
        let layers: Vec<ShownLayer> = layers
            .into_iter()
            .filter(|layer| {
                (layer.fresh && seen)
                    || ran
                        .painting
                        .iter()
                        .any(|placement| placement.screen == layer.placement.screen)
            })
            .collect();
        let mut presented = None;
        if let Some((gpu, format)) = &self.gpu
            && !(ran.painting.is_empty() && layers.is_empty())
        {
            let painted: Vec<u32> = ran
                .painting
                .iter()
                .map(|placement| placement.surface)
                .collect();
            let damage =
                self.panes
                    .paint(&gpu.device, &gpu.queue, *format, screens, ran.painting)?;
            for surface in painted {
                self.fresh.remove(&surface);
            }
            self.panes
                .paint_layers(&gpu.device, &gpu.queue, *format, screens, &layers)?;
            let sequence = PRESENTS.with(|presents| {
                presents.set(presents.get() + 1);
                presents.get()
            });
            presented = Some(PresentedFrame { sequence, damage });
        }
        let mut commit = screens.commit();
        commit.repaint_after_micros = repaint.map(|delay| delay.as_micros() as u64);
        commit.animating = animating;
        commit.presented = presented;
        Ok(Some(commit))
    }
}

impl Surfaces {
    #[cfg(test)]
    pub(crate) fn layer_surfaces(&self) -> Vec<u32> {
        let mut surfaces: Vec<u32> = self.layers.keys().copied().collect();
        surfaces.sort_unstable();
        surfaces
    }

    fn place_layers(&mut self, screens: &Screens) -> Vec<ShownLayer> {
        let mut shown = Vec::new();
        for placement in &self.layout.screens {
            let Some(session) = screens.get(placement.instance) else {
                continue;
            };
            for layer in session.layer_surfaces(placement.region) {
                shown.push(ShownLayer {
                    placement: *placement,
                    layer: *layer,
                    fresh: false,
                });
            }
        }
        let Some((_, format)) = &self.gpu else {
            return shown;
        };
        let format = *format;
        self.layers.retain(|surface, _| {
            let open = shown.iter().any(|layer| layer.layer.surface == *surface);
            if !open {
                block_gpu_guest::release_surface(*surface);
            }
            open
        });
        for layer in &mut shown {
            let [width, height] = layer.layer.pixels;
            if self.layers.get(&layer.layer.surface) == Some(&layer.layer.pixels) {
                continue;
            }
            block_gpu_guest::configure_surface(layer.layer.surface, width, height, format);
            self.layers.insert(layer.layer.surface, layer.layer.pixels);
            layer.fresh = true;
        }
        shown
    }
}

fn format(format: SurfaceFormat) -> wgpu::TextureFormat {
    match format {
        SurfaceFormat::Rgba8Unorm => wgpu::TextureFormat::Rgba8Unorm,
        SurfaceFormat::Rgba8UnormSrgb => wgpu::TextureFormat::Rgba8UnormSrgb,
        SurfaceFormat::Bgra8Unorm => wgpu::TextureFormat::Bgra8Unorm,
        SurfaceFormat::Bgra8UnormSrgb => wgpu::TextureFormat::Bgra8UnormSrgb,
    }
}
