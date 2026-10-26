use std::time::{Duration, Instant};

use beui_adapter_space::head::{Fov, View, Views};
use beui_adapter_space::layers::Allowance;
use beui_adapter_space::{LayerTarget, Space, ViewTarget};
use beui_core::app::Setup;
use beui_core::context::FrameOutput;
use beui_core::geometry::vec2;
use beui_core::runner::{HostView, Hosted, Incoming, WindowHost};
use beui_renderer_wgpu::present::GpuSetup;

use crate::{Frame, Headset, Quad, RETRY};

pub const IDLE_POLL: Duration = Duration::from_millis(100);

pub struct XrSpace {
    pub space: Space,
    headset: Headset,
    gpu: Option<GpuSetup>,
    frame: Option<Frame>,
    paced: bool,
    warned: bool,
}

impl XrSpace {
    pub fn headset(&self) -> &Headset {
        &self.headset
    }

    pub fn new(space: Space, headset: Headset) -> Self {
        Self {
            space,
            headset,
            gpu: None,
            frame: None,
            paced: false,
            warned: false,
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

impl WindowHost for XrSpace {
    fn setup(&mut self, setup: &Setup) {
        self.gpu = setup.get::<GpuSetup>().cloned();
        self.space.setup(setup);
    }

    fn pace(&mut self) -> Option<Instant> {
        let paced = self.headset.pace();
        self.paced = paced.is_some();
        if self.headset.lost() && !std::mem::replace(&mut self.warned, true) {
            let until = match self.headset.returns() {
                true => " until the headset is back",
                false => "",
            };
            eprintln!(
                "beui-adapter-xr: {} stopped; space goes on as the simulator{until}",
                self.headset.runtime()
            );
        }
        paced
    }

    fn input(&mut self, incoming: &mut Vec<Incoming>, view: &HostView) {
        if let Some(gpu) = &self.gpu
            && self.headset.reopen(&gpu.device)
        {
            eprintln!("beui-adapter-xr: the headset is back");
            self.warned = false;
        }
        self.frame = match (&self.gpu, self.paced) {
            (Some(gpu), true) => self.headset.begin(&gpu.device),
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
        self.space.input(incoming, view);
    }

    fn frame(&mut self, output: &FrameOutput, view: &HostView) -> Hosted {
        let mut hosted = self.space.frame(output, view);
        if let (Some(frame), Some(gpu)) = (self.frame.take(), &self.gpu) {
            if let Some(views) = self.headset.relocate() {
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
            self.headset.submit(
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
        if self.paced {
            hosted.repaint_after = Duration::ZERO;
        } else if !self.headset.lost() {
            hosted.repaint_after = hosted.repaint_after.min(IDLE_POLL);
        } else if self.headset.returns() {
            hosted.repaint_after = hosted.repaint_after.min(RETRY);
        }
        hosted
    }
}
