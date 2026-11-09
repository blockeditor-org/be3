use std::time::Duration;

use be_lib_geometry::{Pos2, Rect, Vec2};
#[cfg(target_arch = "wasm32")]
use be_sdk_api::SurfaceRect;
use be_sdk_api::{
    AnchorSpace, AnchoredPlacement, EditorInstanceId, FrameChrome, FrameSpec, TimedInput, View,
    ViewId, ViewRef, WindowRequests,
};
use uuid::Uuid;

use crate::{Artifact, EditorHost};

pub trait App: 'static {
    fn open(host: EditorHost) -> Box<dyn Instance>;
}

pub trait Instance: std::any::Any {
    fn connect(&mut self, block_id: Uuid);

    fn connect_creation(&mut self, _template: String) {}

    fn create_block(&mut self) -> Result<Uuid, String> {
        Err("this editor does not create blocks".into())
    }

    fn connect_artifact(&mut self, _artifact: Artifact) {}

    fn describe_artifact(&mut self, _data: &[u8]) -> Result<String, String> {
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

    fn open_view(&mut self, _view: View) {}
}

pub trait ViewHandler: std::any::Any {
    fn input(&mut self, region: &ShownView, event: &TimedInput, over: Option<ViewRef>);

    fn dismiss_anchored(&mut self, _region: &ShownView, _views: &[ViewId]) {}

    fn run_action(&mut self, _region: &ShownView, _id: &str) {}

    fn node_action(&mut self, _region: &ShownView, _request: be_sdk_api::accesskit::ActionRequest) {
    }

    fn frame(&mut self, regions: &[ShownView]) -> Vec<Frame>;

    fn laid_out(&mut self, _views: &[ViewRef]) {}

    fn frame_opened(&mut self, regions: &[ShownView]) -> Vec<Frame> {
        regions.iter().map(|_| Frame::default()).collect()
    }

    fn painting(&mut self, _region: ViewRef) -> Option<beui_paint_format::Painting> {
        None
    }

    fn overflows(&mut self, _region: ViewRef) -> Vec<String> {
        Vec::new()
    }

    #[cfg(target_arch = "wasm32")]
    fn paint(&mut self, region: ViewRef, target: &PaintTarget<'_>) -> Vec<SurfaceRect>;
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SettingsDraft {
    pub saved: Vec<u8>,
    pub draft: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ShownView {
    pub instance: EditorInstanceId,
    pub view: View,
    pub block: Option<Uuid>,
    pub rect: Rect,
    pub visible: Vec2,
    pub scale_factor: f32,
    pub raster: f32,
    pub pixels: [u32; 2],
    pub age: u32,
    pub spec: FrameSpec,
    pub describe: Option<u64>,
    pub seen: bool,
    pub windows: bool,
    pub fit: bool,
    pub docked: bool,
    pub host_place: Option<String>,
    pub anchor_space: Option<AnchorSpace>,
    pub origin: Option<Pos2>,
}

impl ShownView {
    pub fn id(&self) -> ViewRef {
        ViewRef {
            instance: self.instance,
            view: self.view.id,
        }
    }

    pub fn chrome_drawn(&self) -> bool {
        self.spec.chrome == FrameChrome::Drawn
    }
}

pub fn fitted_pixels(fitted: be_lib_geometry::Vec2, raster: f32) -> (u32, u32) {
    let raster = match raster.is_finite() && raster > 0.0 {
        true => raster,
        false => 1.0,
    };
    let pixels = |size: f32| ((size * raster).ceil() as u32).max(1);
    (pixels(fitted.x), pixels(fitted.y))
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Frame {
    pub changed: bool,
    pub repaint_after: Option<Duration>,
    pub animating: bool,
    pub content: Option<Rect>,
    pub painted: Vec<Rect>,
    pub floating: Vec<Rect>,
    pub floating_painted: Vec<Rect>,
    /// The parts of `floating` that are marks over the view's own children rather than
    /// overlays, and of `floating_painted`, `marks_painted` (`frame-report.marks`).
    pub marks: Vec<Rect>,
    pub marks_painted: Vec<Rect>,
    pub scrims: Vec<be_sdk_api::Scrim>,
    pub drawing: Vec<Rect>,
    pub claims: Vec<Claim>,
    pub work_area: Option<Rect>,
    pub window: WindowRequests,
    pub description: Option<be_sdk_api::Description>,
    /// The view's actions when they changed since its last frame, whether or not the host asks
    /// the app to describe (`frame-report.actions`).
    pub actions: Option<Vec<be_sdk_api::DescribedAction>>,
    pub stopped: Option<be_sdk_api::Stopped>,
    pub asks: Vec<be_sdk_api::WindowRequest>,
    pub anchored: Vec<Anchored>,
    pub fitted: Option<Vec2>,
    /// The opaque color the view is cleared to; see `FrameReport::backdrop`.
    pub backdrop: Option<be_sdk_api::accessibility::Color>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Anchored {
    pub view: ViewId,
    pub placement: AnchoredPlacement,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Claim {
    pub modifiers: be_sdk_api::Modifiers,
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
