use std::time::{Duration, Instant};

use beui_core::app::Setup;
use beui_core::context::{Context, FrameOutput};
use beui_core::geometry::{Pos2, vec2};
use beui_core::runner::{Frame as RunnerFrame, Incoming, Layer, LayerFrame, LayerView, Shown};
use beui_core::window::WindowId;
use beui_layer_space::head::{Fov, View, Views};
use beui_layer_space::layers::Allowance;
use beui_layer_space::notice::Notice;
use beui_layer_space::{Composed, LayerTarget, Space, ViewTarget};
use beui_renderer_wgpu::present::GpuSetup;

use crate::{Frame, Headset, Link, Lost, Quad, RETRY, Status};

pub const IDLE_POLL: Duration = Duration::from_millis(100);
pub const WAITING: &str = "Waiting for a headset";

pub struct XrSpace {
    pub space: Space,
    link: Link,
    headset: Option<Headset>,
    gpu: Option<GpuSetup>,
    frame: Option<Frame>,
    paced: bool,
    warned: Option<Lost>,
}

impl XrSpace {
    pub fn headset(&self) -> Option<&Headset> {
        self.headset.as_ref()
    }

    pub fn link(&self) -> &Link {
        &self.link
    }

    pub fn new(space: Space, link: Link) -> Self {
        Self {
            space,
            link,
            headset: None,
            gpu: None,
            frame: None,
            paced: false,
            warned: None,
        }
    }

    fn adopt(&mut self) {
        let Some(xr) = self.link.opened() else {
            return;
        };
        if self
            .headset
            .as_ref()
            .is_some_and(|headset| headset.xr().same(&xr))
        {
            return;
        }
        self.drop_headset();
        self.headset = Some(xr.headset());
        self.warned = None;
    }

    fn drop_headset(&mut self) {
        if let (Some(gpu), Some(_)) = (&self.gpu, &self.headset) {
            let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
        }
        self.headset = None;
        self.frame = None;
    }

    pub fn notice(&self) -> Option<Notice> {
        let waiting = |detail: String| {
            Some(Notice {
                title: WAITING.to_owned(),
                detail: capitalized(&detail),
            })
        };
        let Some(headset) = &self.headset else {
            return match self.link.status() {
                Status::Missing(why) => waiting(why),
                Status::Found(runtime) => waiting(format!("starting {runtime}")),
                Status::Failed(why) => Some(Notice {
                    title: "No headset".to_owned(),
                    detail: capitalized(&why),
                }),
            };
        };
        match headset.lost() {
            None if headset.running() => None,
            None => waiting(format!(
                "{} has not started this app's session yet",
                headset.runtime()
            )),
            Some(Lost::Session) => waiting(format!("{} lost the headset", headset.runtime())),
            Some(Lost::Instance) => waiting(format!("{} stopped", headset.runtime())),
            Some(Lost::Ended) => None,
        }
    }

    fn views(display_time: Instant, views: &[crate::View]) -> Views {
        Views {
            display_time,
            views: views
                .iter()
                .map(|view| View {
                    position: view.position,
                    rotation: view.orientation,
                    fov: Fov::from_angles(
                        view.fov.left,
                        view.fov.right,
                        view.fov.up,
                        view.fov.down,
                    ),
                    size: vec2(view.size.0 as f32, view.size.1 as f32),
                })
                .collect(),
        }
    }
}

impl Layer for XrSpace {
    fn attach(&mut self, context: &Context) {
        self.space.attach(context);
    }

    fn setup(&mut self, setup: &Setup) {
        XrSpace::setup(self, setup);
    }

    fn pace(&mut self) -> Option<Instant> {
        XrSpace::pace(self)
    }

    fn input(&mut self, incoming: &mut Vec<Incoming>, layer: &mut LayerFrame<'_>) {
        XrSpace::input(self, incoming, &layer.view);
    }

    fn keyboard_attached(&self) -> Option<bool> {
        self.space.keyboard_attached()
    }

    fn output(&mut self, shown: &mut Shown, output: &FrameOutput, layer: &mut LayerFrame<'_>) {
        Layer::output(&mut self.space, shown, output, layer);
        self.show();
    }

    fn framed(
        &mut self,
        frame: &mut RunnerFrame,
        output: &FrameOutput,
        layer: &mut LayerFrame<'_>,
    ) {
        self.space.framed(frame, output, layer);
        frame.repaint_after = frame.repaint_after.min(self.wait());
    }

    fn up(&self, window: WindowId, at: Pos2) -> Option<(WindowId, Pos2)> {
        self.space.up(window, at)
    }

    fn down(&self, window: WindowId, at: Pos2) -> Option<(WindowId, Pos2)> {
        self.space.down(window, at)
    }
}

impl XrSpace {
    pub fn setup(&mut self, setup: &Setup) {
        self.gpu = setup.get::<GpuSetup>().cloned();
        self.space.setup(setup);
        self.link.set_waker(setup.waker.clone());
        self.adopt();
    }

    pub fn pace(&mut self) -> Option<Instant> {
        let Some(headset) = &mut self.headset else {
            self.paced = false;
            return None;
        };
        let paced = headset.pace();
        self.paced = paced.is_some();
        let lost = headset.lost();
        if lost != self.warned {
            let runtime = headset.runtime();
            match lost {
                Some(Lost::Session) => {
                    eprintln!("beui-layer-xr: {runtime} lost the headset; waiting for it")
                }
                Some(Lost::Instance) => {
                    eprintln!("beui-layer-xr: {runtime} stopped; waiting for a headset")
                }
                Some(Lost::Ended) => eprintln!(
                    "beui-layer-xr: {runtime} ended the session; space goes on as the simulator"
                ),
                None => {}
            }
            self.warned = lost;
        }
        if lost == Some(Lost::Instance) {
            let xr = headset.xr().clone();
            self.drop_headset();
            self.link.lost(&xr);
        }
        paced
    }

    pub fn input(&mut self, incoming: &mut Vec<Incoming>, view: &LayerView) {
        if let (Some(gpu), Some(headset)) = (&self.gpu, &mut self.headset)
            && headset.reopen(&gpu.device)
        {
            eprintln!("beui-layer-xr: the headset is back");
        }
        self.frame = match (&self.gpu, &mut self.headset, self.paced) {
            (Some(gpu), Some(headset), true) => headset.begin(&gpu.device),
            _ => None,
        };
        if let Some(frame) = &self.frame {
            incoming.extend(frame.controllers.iter().copied().map(Incoming::Controller));
        }
        let views = self
            .frame
            .as_ref()
            .map(|frame| Self::views(frame.display, &frame.views));
        if views.is_some() || self.space.tracked().is_some() {
            self.space.set_views(views);
        }
        self.space.set_notice(self.notice());
        self.space.input(incoming, view);
    }

    pub fn frame(&mut self, output: &FrameOutput, view: &LayerView) -> Composed {
        let mut composed = self.space.frame(output, view);
        self.show();
        composed.repaint_after = composed.repaint_after.min(self.wait());
        composed
    }

    fn show(&mut self) {
        if let (Some(frame), Some(gpu), Some(headset)) =
            (self.frame.take(), &self.gpu, &mut self.headset)
        {
            if let Some(views) = headset.relocate() {
                self.space
                    .set_views(Some(Self::views(frame.display, &views)));
            }
            let commands = gpu
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("xr views"),
                });
            let allowance = Allowance {
                most: frame.layers.most,
                cylinders: frame.layers.cylinders,
                largest: frame.layers.largest,
            };
            let quads: Vec<Quad> = self
                .space
                .layers(&allowance)
                .into_iter()
                .map(|layer| Quad {
                    window: layer.window,
                    position: layer.position,
                    orientation: layer.rotation,
                    metres: (layer.metres.x, layer.metres.y),
                    bend: layer.bend,
                    pixels: layer.pixels,
                    format: layer.format,
                    version: layer.version,
                })
                .collect();
            let space = &mut self.space;
            headset.submit(
                &gpu.device,
                &gpu.queue,
                commands,
                &quads,
                &mut |targets, fills, commands| {
                    let views: Vec<wgpu::TextureView> = targets
                        .iter()
                        .map(|target| {
                            target
                                .texture
                                .create_view(&wgpu::TextureViewDescriptor::default())
                        })
                        .collect();
                    let targets: Vec<ViewTarget<'_>> = targets
                        .iter()
                        .zip(&views)
                        .map(|(target, texture)| ViewTarget {
                            view: target.view,
                            texture,
                            format: frame.format,
                        })
                        .collect();
                    let filled: Vec<Option<wgpu::TextureView>> = fills
                        .iter()
                        .map(|fill| {
                            fill.texture.map(|texture| {
                                texture.create_view(&wgpu::TextureViewDescriptor::default())
                            })
                        })
                        .collect();
                    let layers: Vec<LayerTarget<'_>> = fills
                        .iter()
                        .zip(&filled)
                        .map(|(fill, texture)| LayerTarget {
                            window: quads[fill.quad].window,
                            fill: texture
                                .as_ref()
                                .zip(fill.texture.map(|texture| texture.format())),
                        })
                        .collect();
                    space.draw(commands, &targets, &layers);
                },
            );
        }
    }

    fn wait(&self) -> Duration {
        if self.paced {
            return Duration::ZERO;
        }
        match self.headset.as_ref().map(Headset::lost) {
            Some(None) => IDLE_POLL,
            Some(Some(Lost::Session)) => RETRY,
            Some(Some(_)) | None => Duration::MAX,
        }
    }
}

fn capitalized(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}
