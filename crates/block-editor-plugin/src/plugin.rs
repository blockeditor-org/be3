use std::time::Duration;

#[cfg(target_arch = "wasm32")]
use block_plugin_api::SurfaceRect;
use block_plugin_api::{EditorRegion, FrameChrome, FrameSpec, TimedInput, WindowRequests};
use geometry::{Rect, Vec2};
use uuid::Uuid;

use crate::{Artifact, ArtifactDescription, EditorHost};

pub trait Plugin: 'static {
    fn open(host: EditorHost) -> Box<dyn Instance>;
}

pub trait Instance: std::any::Any {
    fn connect(&mut self, block_id: Uuid);

    fn connect_creation(&mut self, _template: String) {}

    fn create_block(&mut self) -> Result<Uuid, String> {
        Err("this editor does not create blocks".into())
    }

    fn connect_artifact(&mut self, _artifact: Artifact) {}

    fn describe_artifact(&mut self, _data: &[u8]) -> Result<ArtifactDescription, String> {
        Err("this editor does not generate artifacts".into())
    }

    fn regenerate_artifact(&mut self, _data: &[u8]) {}

    fn poll_artifact(&mut self) -> Option<Result<(), String>> {
        None
    }

    fn intrinsic_size(&mut self) -> Option<Vec2> {
        None
    }

    fn resized(&mut self, _size: Vec2) {}

    fn aspect_ratio(&mut self) -> Option<f32> {
        None
    }

    fn presence_visible(&mut self, _visible: bool) {}

    fn replace_child(&mut self, _old: Uuid, _new: Uuid) -> bool {
        false
    }

    fn input(&mut self, region: &Region, event: &TimedInput);

    fn run_action(&mut self, _region: &Region, _id: &str) {}

    fn node_action(
        &mut self,
        _region: &Region,
        _request: block_plugin_api::accesskit::ActionRequest,
    ) {
    }

    fn update(&mut self, region: &Region, settings: Option<SettingsDraft<'_>>) -> Frame;

    fn painting(&mut self, _region: EditorRegion) -> Option<paint_format::Painting> {
        None
    }

    fn overflows(&mut self, _region: EditorRegion) -> Vec<String> {
        Vec::new()
    }

    #[cfg(target_arch = "wasm32")]
    fn paint(&mut self, target: &PaintTarget<'_>) -> Vec<SurfaceRect>;
}

pub struct SettingsDraft<'a> {
    pub saved: &'a [u8],
    pub draft: &'a mut Vec<u8>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Region {
    pub region: EditorRegion,
    pub rect: Rect,
    pub scale_factor: f32,
    pub pixels: [u32; 2],
    pub age: u32,
    pub spec: FrameSpec,
    pub monitors: Vec<RegionMonitor>,
    pub describe: Option<u64>,
    pub seen: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RegionMonitor {
    pub id: String,
    pub name: String,
    pub rect: Rect,
}

impl Region {
    pub fn chrome_drawn(&self) -> bool {
        self.spec.chrome == FrameChrome::Drawn
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Frame {
    pub changed: bool,
    pub repaint_after: Option<Duration>,
    pub animating: bool,
    pub content: Option<Rect>,
    pub painted: Vec<Rect>,
    pub floating: Vec<Rect>,
    pub drawing: Vec<Rect>,
    pub every_sample: bool,
    pub claims: Vec<Claim>,
    pub window: WindowRequests,
    pub description: Option<block_plugin_api::Description>,
    pub stopped: Option<block_plugin_api::Stopped>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Claim {
    pub modifiers: block_plugin_api::Modifiers,
    pub rect: Rect,
}

#[cfg(target_arch = "wasm32")]
pub struct PaintTarget<'a> {
    pub device: &'a wgpu::Device,
    pub queue: &'a wgpu::Queue,
    pub view: &'a wgpu::TextureView,
    pub format: wgpu::TextureFormat,
    pub width: u32,
    pub height: u32,
    pub region: EditorRegion,
    pub age: u32,
}

#[cfg(target_arch = "wasm32")]
impl PaintTarget<'_> {
    pub fn whole(&self) -> Vec<SurfaceRect> {
        vec![SurfaceRect {
            x: 0,
            y: 0,
            width: self.width,
            height: self.height,
        }]
    }
}

#[cfg(target_arch = "wasm32")]
#[derive(Clone)]
pub struct SurfaceGpu {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub format: wgpu::TextureFormat,
}

#[cfg(target_arch = "wasm32")]
pub fn surface_gpu() -> Option<SurfaceGpu> {
    crate::wasm::surface_gpu()
}
