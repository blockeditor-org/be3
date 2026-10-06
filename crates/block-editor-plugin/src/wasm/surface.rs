use std::cell::{Cell, RefCell};
use std::collections::HashSet;

use block_plugin_api::{
    FrameReady, Message, PresentedFrame, ScreenLayout, SurfaceFormat, SurfaceSpec,
};

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

pub(crate) fn initialize() -> Result<(), String> {
    let (device, queue) = block_gpu_guest::device_and_queue();
    GPU.with(|gpu| *gpu.borrow_mut() = Some(Gpu { device, queue }));
    Ok(())
}

fn gpu() -> Result<Gpu, String> {
    GPU.with(|gpu| gpu.borrow().clone())
        .ok_or_else(|| "the plugin gpu is not ready".to_owned())
}

pub(crate) fn surface_gpu() -> Option<crate::SurfaceGpu> {
    let Gpu { device, queue } = gpu().ok()?;
    Some(crate::SurfaceGpu {
        device,
        queue,
        format: FORMAT.get()?,
    })
}

pub(crate) struct Surfaces {
    gpu: Gpu,
    format: wgpu::TextureFormat,
    panes: Panes,
    layout: ScreenLayout,
    fresh: HashSet<u32>,
}

impl Surfaces {
    pub(crate) fn new(spec: SurfaceSpec) -> Result<Self, String> {
        FORMAT.set(Some(format(spec.format)));
        Ok(Self {
            gpu: gpu()?,
            format: format(spec.format),
            panes: Panes::default(),
            layout: ScreenLayout::default(),
            fresh: HashSet::new(),
        })
    }

    pub(crate) fn layout(&self) -> &ScreenLayout {
        &self.layout
    }

    pub(crate) fn place(&mut self, layout: ScreenLayout) {
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
                    self.format,
                );
                self.fresh.insert(placement.surface);
            }
        }
        self.layout = layout;
    }

    pub(crate) fn render(&mut self, screens: &mut Screens) -> Result<Vec<Message>, String> {
        if self.layout.is_empty() {
            return Ok(Vec::new());
        }
        let ran = self.panes.run(&self.layout, screens, &self.fresh);
        let repaint = ran.repaint;
        let mut presented = None;
        if !ran.painting.is_empty() {
            let painted: Vec<u32> = ran
                .painting
                .iter()
                .map(|placement| placement.surface)
                .collect();
            let damage = self.panes.paint(
                &self.gpu.device,
                &self.gpu.queue,
                self.format,
                screens,
                ran.painting,
            )?;
            for surface in painted {
                self.fresh.remove(&surface);
            }
            let sequence = PRESENTS.with(|presents| {
                presents.set(presents.get() + 1);
                presents.get()
            });
            presented = Some(PresentedFrame { sequence, damage });
        }
        Ok(vec![Message::FrameReady(FrameReady {
            generation: self.layout.generation,
            repaint_after_micros: repaint.map(|delay| delay.as_micros() as u64),
            presented,
        })])
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
