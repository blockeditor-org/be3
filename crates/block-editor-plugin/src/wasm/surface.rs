use stable_hash::HashSet;
use std::cell::{Cell, RefCell};

use block_plugin_api::{FrameCommit, PresentedFrame, ScreenLayout, SurfaceFormat, SurfaceSpec};

use crate::{panes::Panes, screens::Screens};

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
            let requested = block_gpu_guest::request_device(&wgpu::DeviceDescriptor {
                label: Some("plugin device"),
                required_limits: block_gpu_guest::limits(),
                ..Default::default()
            });
            *gpu = requested.ok().map(|(device, queue)| Gpu { device, queue });
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
    relaid: bool,
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
            fresh: HashSet::default(),
            relaid: false,
        })
    }

    pub(crate) fn layout(&self) -> &ScreenLayout {
        &self.layout
    }

    pub(crate) fn take_relaid(&mut self) -> bool {
        std::mem::take(&mut self.relaid)
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
        if !self.layout.same_placements(screens.layout()) {
            self.place(screens.layout().clone());
            self.relaid = true;
        }
        let mut presented = None;
        if let Some((gpu, format)) = &self.gpu
            && !ran.painting.is_empty()
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

fn format(format: SurfaceFormat) -> wgpu::TextureFormat {
    match format {
        SurfaceFormat::Rgba8Unorm => wgpu::TextureFormat::Rgba8Unorm,
        SurfaceFormat::Rgba8UnormSrgb => wgpu::TextureFormat::Rgba8UnormSrgb,
        SurfaceFormat::Bgra8Unorm => wgpu::TextureFormat::Bgra8Unorm,
        SurfaceFormat::Bgra8UnormSrgb => wgpu::TextureFormat::Bgra8UnormSrgb,
    }
}
