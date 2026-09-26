use std::time::Duration;

use block_plugin_api::{
    CursorIcon, EditorRegion, FrameChrome, FrameSpec, InputEvent, ScreenPlacement,
};
use geometry::{Rect, Vec2};
use uuid::Uuid;

use crate::{Artifact, ArtifactDescription, EditorHost};

pub trait Plugin: 'static {
    fn open(host: EditorHost) -> Box<dyn Instance>;
}

pub trait Instance {
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

    fn input(&mut self, region: &Region, event: &InputEvent);

    fn update(&mut self, region: &Region, settings: Option<&mut Vec<u8>>) -> Frame;

    fn paint(&mut self, target: &PaintTarget<'_>);
}

#[derive(Clone, Debug, PartialEq)]
pub struct Region {
    pub region: EditorRegion,
    pub rect: Rect,
    pub scale_factor: f32,
    pub spec: FrameSpec,
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
    pub cursor: CursorIcon,
    pub content: Option<Rect>,
    pub painted: Vec<Rect>,
    pub floating: Vec<Rect>,
    pub ime: Option<Ime>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ime {
    pub rect: Rect,
    pub cursor: Rect,
}

pub struct PaintTarget<'a> {
    pub device: &'a wgpu::Device,
    pub queue: &'a wgpu::Queue,
    pub view: &'a wgpu::TextureView,
    pub format: wgpu::TextureFormat,
    pub width: u32,
    pub height: u32,
    pub placement: ScreenPlacement,
}

impl PaintTarget<'_> {
    pub fn scissor(&self) -> (u32, u32, u32, u32) {
        let placement = self.placement;
        let x = placement.x.min(self.width);
        let y = placement.y.min(self.height);
        (
            x,
            y,
            placement.width.min(self.width - x),
            placement.height.min(self.height - y),
        )
    }
}
