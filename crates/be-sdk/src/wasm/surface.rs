use be_lib_stable_hash::HashSet;
use std::cell::{Cell, RefCell};

use be_sdk_api::{
    LayoutCommit, PresentCommit, PresentedFrame, ScreenLayout, SurfaceFormat, SurfaceSpec,
};

use crate::{
    panes::{Panes, Ran},
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
            let requested = be_wasm_gpu_guest::request_device(&wgpu::DeviceDescriptor {
                label: Some("app device"),
                required_limits: be_wasm_gpu_guest::limits(),
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
    /// A surface placed anew in the frame that laid it out, whose painting was dropped (`lay_out`),
    /// is owed a frame at its new size.
    owed: bool,
}

impl Surfaces {
    pub(crate) fn new(spec: SurfaceSpec) -> Result<Self, String> {
        let gpu = match spec.render {
            true => {
                FORMAT.set(Some(format(spec.format)));
                Some((
                    gpu().ok_or("the app gpu is not ready")?,
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
            owed: false,
        })
    }

    pub(crate) fn layout(&self) -> &ScreenLayout {
        &self.layout
    }

    pub(crate) fn take_relaid(&mut self) -> bool {
        std::mem::take(&mut self.relaid)
    }

    /// Whether a surface the last `lay_out` placed anew is left without a frame at its new size,
    /// which the app asks the host for (`frame-needed`), so no host need know to lay it out again.
    pub(crate) fn take_owed(&mut self) -> bool {
        std::mem::take(&mut self.owed)
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
                be_wasm_gpu_guest::release_surface(gone.surface);
                self.fresh.remove(&gone.surface);
            }
        }
        for placement in &layout.screens {
            let kept = self.layout.screens.iter().any(|previous| {
                previous.surface == placement.surface
                    && (previous.width, previous.height) == (placement.width, placement.height)
            });
            if !kept {
                be_wasm_gpu_guest::configure_surface(
                    placement.surface,
                    placement.width,
                    placement.height,
                    format,
                    &[],
                    // beui draws premultiplied, and a surface may be translucent anywhere.
                    wgpu::CompositeAlphaMode::PreMultiplied,
                );
                self.fresh.insert(placement.surface);
            }
        }
        self.layout = layout;
    }

    pub(crate) fn lay_out(&mut self, screens: &mut Screens) -> Option<(LayoutCommit, Ran)> {
        if self.layout.is_empty() {
            return None;
        }
        let ran = self.panes.run(
            &self.layout,
            screens,
            &self.fresh,
            crate::screens::seen(),
            self.gpu.is_some(),
        );
        let mut ran = ran;
        if !self.layout.same_placements(screens.layout()) {
            self.place(screens.layout().clone());
            self.relaid = true;
            // A view that fitted itself to new content in this frame was run at the placement it
            // had: its painting is at a size its surface no longer has, so it is not presented,
            // and the surface, fresh, is drawn at its new size in the next frame, which it owes.
            let before = ran.painting.len();
            let layout = &self.layout;
            ran.painting.retain(|painting| {
                layout.screens.iter().any(|laid| {
                    laid.surface == painting.surface
                        && (laid.width, laid.height) == (painting.width, painting.height)
                })
            });
            self.owed |= ran.painting.len() != before;
        }
        Some((screens.commit(), ran))
    }

    pub(crate) fn paint(
        &mut self,
        screens: &mut Screens,
        serial: u64,
        ran: Ran,
    ) -> Result<PresentCommit, String> {
        let repaint = ran.repaint;
        let animating = ran.animating;
        let input_waiting = ran.input_waiting;
        let mut presented = None;
        if let Some((gpu, format)) = &self.gpu
            && !ran.painting.is_empty()
        {
            let painted = ran.painting.clone();
            let damage =
                self.panes
                    .paint(&gpu.device, &gpu.queue, *format, screens, ran.painting)?;
            for placement in &painted {
                self.fresh.remove(&placement.surface);
            }
            let sequence = PRESENTS.with(|presents| {
                presents.set(presents.get() + 1);
                presents.get()
            });
            presented = Some(PresentedFrame {
                sequence,
                damage,
                painted,
            });
        }
        Ok(PresentCommit {
            serial,
            repaint_after_micros: repaint.map(|delay| delay.as_micros() as u64),
            animating,
            presented,
            input_waiting,
        })
    }
}

fn format(format: SurfaceFormat) -> wgpu::TextureFormat {
    match format {
        SurfaceFormat::Rgba8Unorm => wgpu::TextureFormat::Rgba8Unorm,
        SurfaceFormat::Bgra8Unorm => wgpu::TextureFormat::Bgra8Unorm,
    }
}
