mod services;
mod surface;
#[cfg(test)]
mod tests;

use std::collections::HashSet;
use std::time::Duration;

#[cfg(target_arch = "wasm32")]
use block_plugin_api::SurfaceRect;
use block_plugin_api::accesskit::ActionRequest;
use block_plugin_api::{Stopped, TimedInput, ViewId, ViewRef, WindowRequest};

use crate::{EditorHost, Frame, ShownView, ViewHandler};
#[cfg(target_arch = "wasm32")]
use crate::{PaintTarget, SurfaceGpu};

pub use services::HostServices;
pub use surface::{Damage, Surface, Surfaces};

pub trait Regions: 'static {
    type Painter: Default + 'static;

    fn input(
        &mut self,
        services: &mut HostServices,
        region: &ShownView,
        event: &TimedInput,
        over: Option<ViewRef>,
    );

    fn dismiss_anchored(&mut self, _region: &ShownView, _views: &[ViewId]) {}

    fn run_action(&mut self, _region: &ShownView, _id: &str) {}

    fn node_action(&mut self, _region: &ShownView, _request: ActionRequest) {}

    fn owner(&self, _view: ViewRef) -> Option<ViewRef> {
        None
    }

    fn frame(
        &mut self,
        services: &mut HostServices,
        active: Option<ViewRef>,
        regions: &[ShownView],
    ) -> Result<Ran, Option<Stopped>>;

    fn opened(&mut self, _regions: &[ShownView]) {}

    fn framed(
        &mut self,
        surfaces: &mut Surfaces<Self::Painter>,
        region: &ShownView,
        keyboard: Option<ViewRef>,
    ) -> Frame;

    fn anchored_frame(&mut self, _region: &ShownView) -> Frame {
        Frame::default()
    }

    fn anchored(&mut self, _view: ViewRef) -> Option<&mut Surface<Self::Painter>> {
        None
    }

    fn painting(
        &mut self,
        _surfaces: &Surfaces<Self::Painter>,
        _region: ViewRef,
    ) -> Option<paint_format::Painting> {
        None
    }

    fn overflows(&mut self, _region: ViewRef) -> Vec<String> {
        Vec::new()
    }

    #[cfg(target_arch = "wasm32")]
    fn paint(
        painter: &mut Self::Painter,
        gpu: &SurfaceGpu,
        target: &PaintTarget<'_>,
        size: (u32, u32),
        repaint: Damage,
    ) -> Option<Vec<SurfaceRect>>;
}

#[derive(Default)]
pub struct Ran {
    pub keyboard: Option<ViewRef>,
    pub asks: Vec<(ViewRef, WindowRequest)>,
    pub repaint_after: Option<Duration>,
    pub animating: bool,
}

pub struct Driver<R: Regions> {
    regions: R,
    services: HostServices,
    surfaces: Surfaces<R::Painter>,
    active: Option<ViewRef>,
    #[cfg(target_arch = "wasm32")]
    gpu: Option<SurfaceGpu>,
}

impl<R: Regions> Driver<R> {
    pub fn new(host: EditorHost, regions: R) -> Self {
        Self {
            regions,
            services: HostServices::new(host),
            surfaces: Surfaces::default(),
            active: None,
            #[cfg(target_arch = "wasm32")]
            gpu: None,
        }
    }

    pub fn of(handler: &mut dyn ViewHandler) -> Option<&mut Self> {
        (handler as &mut dyn std::any::Any).downcast_mut()
    }

    pub fn regions(&self) -> &R {
        &self.regions
    }

    pub fn regions_mut(&mut self) -> &mut R {
        &mut self.regions
    }

    pub fn surfaces(&self) -> &Surfaces<R::Painter> {
        &self.surfaces
    }

    fn finish(&mut self, id: ViewRef, mut frame: Frame) -> Frame {
        let surface = match self.regions.anchored(id) {
            Some(surface) => Some(surface),
            None => self.surfaces.get_mut(id),
        };
        if let Some(surface) = surface {
            frame.changed |= surface.unpainted();
            surface.settle();
        }
        frame
    }
}

impl<R: Regions> ViewHandler for Driver<R> {
    fn input(&mut self, region: &ShownView, event: &TimedInput, over: Option<ViewRef>) {
        let id = region.id();
        self.active = Some(self.regions.owner(id).unwrap_or(id));
        self.services.serve(region.instance);
        self.regions.input(&mut self.services, region, event, over);
    }

    fn dismiss_anchored(&mut self, region: &ShownView, views: &[ViewId]) {
        self.services.serve(region.instance);
        self.regions.dismiss_anchored(region, views);
    }

    fn run_action(&mut self, region: &ShownView, id: &str) {
        self.regions.run_action(region, id);
    }

    fn node_action(&mut self, region: &ShownView, request: ActionRequest) {
        self.services.serve(region.instance);
        self.regions.node_action(region, request);
    }

    fn frame(&mut self, regions: &[ShownView]) -> Vec<Frame> {
        let shown: HashSet<ViewRef> = regions.iter().map(ShownView::id).collect();
        self.surfaces.retain(|id| shown.contains(&id));
        for region in regions {
            self.surfaces.keep(region);
        }
        let ran = match self.regions.frame(&mut self.services, self.active, regions) {
            Ok(ran) => ran,
            Err(stopped) => {
                return regions
                    .iter()
                    .map(|_| Frame {
                        stopped: stopped.clone(),
                        ..Frame::default()
                    })
                    .collect();
            }
        };
        let keyboard = ran
            .keyboard
            .or(self.active)
            .filter(|id| shown.contains(id))
            .or_else(|| regions.first().map(ShownView::id));
        let fallback = regions.first().map(ShownView::id);
        let mut asks = ran.asks;
        regions
            .iter()
            .map(|region| {
                let id = region.id();
                let mut mine = Vec::new();
                asks.retain(|(from, ask)| {
                    let owner = regions
                        .iter()
                        .map(ShownView::id)
                        .find(|region| region.instance == from.instance)
                        .or(fallback);
                    let here = owner == Some(id);
                    if here {
                        mine.push(ask.clone());
                    }
                    !here
                });
                let frame = Frame {
                    repaint_after: ran.repaint_after.filter(|_| keyboard == Some(id)),
                    animating: ran.animating,
                    asks: mine,
                    ..self.regions.framed(&mut self.surfaces, region, keyboard)
                };
                self.finish(id, frame)
            })
            .collect()
    }

    fn frame_opened(&mut self, regions: &[ShownView]) -> Vec<Frame> {
        for region in regions.iter().filter(|region| !region.view.is_anchored()) {
            self.surfaces.keep(region);
        }
        self.regions.opened(regions);
        regions
            .iter()
            .map(|region| {
                let frame = match region.view.is_anchored() {
                    true => self.regions.anchored_frame(region),
                    false => self.regions.framed(&mut self.surfaces, region, None),
                };
                self.finish(region.id(), frame)
            })
            .collect()
    }

    fn painting(&mut self, region: ViewRef) -> Option<paint_format::Painting> {
        self.regions.painting(&self.surfaces, region)
    }

    fn overflows(&mut self, region: ViewRef) -> Vec<String> {
        self.regions.overflows(region)
    }

    #[cfg(target_arch = "wasm32")]
    fn paint(&mut self, region: ViewRef, target: &PaintTarget<'_>) -> Vec<SurfaceRect> {
        if self.gpu.is_none() {
            self.gpu = crate::surface_gpu();
        }
        let surface = match self.regions.anchored(region) {
            Some(surface) => {
                surface.size = (target.width, target.height);
                surface
            }
            None => match self.surfaces.get_mut(region) {
                Some(surface) => surface,
                None => return Vec::new(),
            },
        };
        surface.paint(target, |painter, target, size, repaint| {
            R::paint(painter, self.gpu.as_ref()?, target, size, repaint)
        })
    }
}
