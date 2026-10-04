use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, Mutex},
};

use beui::{DrawAt, Pos2, Rect, Vec2, pos2, vec2};
use block_plugin_api::{ScreenId, ScreenLayout, ScreenPlacement, SurfaceRect};

use super::backend::{Availability, Frame, ShownFrame};

#[cfg(not(target_arch = "wasm32"))]
use super::surface::{Presenter as PlatformPresenter, presenter as build_presenter};
#[cfg(target_arch = "wasm32")]
use super::web::presenter::{Presenter as PlatformPresenter, presenter as build_presenter};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum PresenterState {
    Waiting,
    Presenting,
    Unsupported(String),
    Failed(String),
    Released,
}

#[derive(Clone)]
pub(super) struct PresenterStatus(Arc<Mutex<PresenterState>>);

impl PresenterStatus {
    pub(super) fn waiting() -> Self {
        Self(Arc::new(Mutex::new(PresenterState::Waiting)))
    }

    pub(super) fn get(&self) -> PresenterState {
        self.0.lock().unwrap().clone()
    }

    fn set(&self, state: PresenterState) {
        *self.0.lock().unwrap() = state;
    }
}

pub(super) trait SurfacePresenter {
    type Frame;

    fn replace(
        &mut self,
        device: &wgpu::Device,
        surface: u32,
        frame: &Self::Frame,
    ) -> Result<(), String>;

    fn prepare(
        &mut self,
        queue: &wgpu::Queue,
        surface: u32,
        frame: &Self::Frame,
    ) -> Result<(), String>;

    fn paint(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        surface: u32,
        regions: &wgpu::BindGroup,
        offset: u32,
    );

    fn release(&mut self, surface: u32);
}

pub(super) const MAX_SURFACES: u32 = 8;
const MAX_PENDING_FRAMES: usize = 8;
pub(super) const REGION_BYTES: u64 = 64;

pub(super) struct RegionLayout {
    pub(super) layout: wgpu::BindGroupLayout,
    stride: u32,
}

impl RegionLayout {
    pub(super) fn new(device: &wgpu::Device) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("hosted plugin region layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: wgpu::BufferSize::new(REGION_BYTES),
                },
                count: None,
            }],
        });
        let stride = device
            .limits()
            .min_uniform_buffer_offset_alignment
            .max(REGION_BYTES as u32);
        Self { layout, stride }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Quad {
    pub(crate) rect: Rect,
    pub(crate) corners: [Pos2; 4],
    pub(crate) opacity: f32,
}

impl Quad {
    pub(crate) fn upright(rect: Rect) -> Self {
        Self {
            rect,
            corners: [
                rect.left_top(),
                rect.right_top(),
                rect.right_bottom(),
                rect.left_bottom(),
            ],
            opacity: 1.0,
        }
    }

    pub(crate) fn crop_to(self, clip: Rect) -> Option<(Self, Rect)> {
        let horizontal = self.corners[1] - self.corners[0];
        let vertical = self.corners[3] - self.corners[0];
        let determinant = horizontal.x * vertical.y - horizontal.y * vertical.x;
        if determinant.abs() <= f32::EPSILON {
            return None;
        }
        let to_uv = |point: Pos2| {
            let delta = point - self.corners[0];
            pos2(
                (delta.x * vertical.y - delta.y * vertical.x) / determinant,
                (horizontal.x * delta.y - horizontal.y * delta.x) / determinant,
            )
        };
        let source = Rect::from_points(&[
            to_uv(clip.left_top()),
            to_uv(clip.right_top()),
            to_uv(clip.right_bottom()),
            to_uv(clip.left_bottom()),
        ])
        .intersect(Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)));
        if source.width() <= 0.0 || source.height() <= 0.0 {
            return None;
        }
        let point = |u: f32, v: f32| self.corners[0] + horizontal * u + vertical * v;
        let corners = [
            point(source.min.x, source.min.y),
            point(source.max.x, source.min.y),
            point(source.max.x, source.max.y),
            point(source.min.x, source.max.y),
        ];
        Some((
            Self {
                rect: Rect::from_points(&corners),
                corners,
                opacity: self.opacity,
            },
            source,
        ))
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Region {
    pub(super) offset: [f32; 2],
    pub(super) scale: [f32; 2],
    pub(super) quad: Quad,
}

impl Region {
    pub(super) fn of(
        layout: &ScreenLayout,
        screen: ScreenId,
        quad: Quad,
        source: Rect,
    ) -> Option<Self> {
        if layout.is_empty() {
            return None;
        }
        let placement = layout.placement(screen)?;
        let width = layout.width as f32;
        let height = layout.height as f32;
        let left = placement.x as f32 + placement.width as f32 * source.min.x;
        let top = placement.y as f32 + placement.height as f32 * source.min.y;
        Some(Self {
            offset: [left / width, top / height],
            scale: [
                placement.width as f32 * source.width() / width,
                placement.height as f32 * source.height() / height,
            ],
            quad,
        })
    }

    fn values(&self, at: &DrawAt) -> [f32; 16] {
        let scale = at.pixels_per_point;
        let screen: Vec2 = vec2(at.screen.x.max(1.0), at.screen.y.max(1.0));
        let mut values = [0.0; 16];
        values[..2].copy_from_slice(&self.offset);
        values[2..4].copy_from_slice(&self.scale);
        for (index, corner) in self.quad.corners.iter().enumerate() {
            values[4 + index * 2] = corner.x * scale / screen.x * 2.0 - 1.0;
            values[5 + index * 2] = 1.0 - corner.y * scale / screen.y * 2.0;
        }
        values[12] = self.quad.opacity.clamp(0.0, 1.0);
        values
    }
}

#[derive(Default)]
pub(super) struct Shared {
    pub(super) layout: ScreenLayout,
    pub(super) frames: Vec<Frame>,
    damage: Option<Vec<SurfaceRect>>,
}

impl Shared {
    pub(super) fn publish(&mut self, layout: &ScreenLayout, frame: Option<Frame>) {
        self.layout.clone_from(layout);
        let Some(frame) = frame else {
            return;
        };
        self.damage = match (self.frames.is_empty(), self.damage.take(), frame.damage()) {
            (true, _, damage) => damage.map(<[SurfaceRect]>::to_vec),
            (false, Some(mut held), Some(damage)) => {
                held.extend_from_slice(damage);
                Some(held)
            }
            _ => None,
        };
        if self.frames.len() >= MAX_PENDING_FRAMES {
            self.frames.remove(0);
        }
        self.frames.push(frame);
    }

    fn take_frames(&mut self) -> Vec<Frame> {
        self.damage = None;
        std::mem::take(&mut self.frames)
    }
}

#[derive(Clone)]
pub(crate) struct Blit {
    pub(super) surface: u32,
    pub(super) status: PresenterStatus,
    pub(super) shared: Rc<RefCell<Shared>>,
    pub(super) screen: ScreenId,
    pub(super) quad: Quad,
    pub(super) source: Rect,
    pub(super) drawn: Option<(u32, u32)>,
    pub(super) requested: Option<(u32, u32)>,
    pub(super) placed: Option<[u32; 4]>,
}

impl PartialEq for Blit {
    fn eq(&self, other: &Self) -> bool {
        self.surface == other.surface
            && Rc::ptr_eq(&self.shared, &other.shared)
            && self.screen == other.screen
            && self.quad == other.quad
            && self.source == other.source
            && self.drawn == other.drawn
            && self.requested == other.requested
            && self.placed == other.placed
    }
}

impl Blit {
    pub(super) fn damage(&self, pieces: &[Piece]) -> Option<Vec<Rect>> {
        let shared = self.shared.borrow();
        if shared.frames.is_empty() {
            return Some(Vec::new());
        }
        let damage = shared.damage.as_ref()?;
        let placement = shared.layout.placement(self.screen)?;
        if placement.width == 0
            || placement.height == 0
            || self
                .requested
                .is_some_and(|requested| requested != (placement.width, placement.height))
        {
            return None;
        }
        Some(
            damage
                .iter()
                .flat_map(|rect| damaged_pieces(*rect, *placement, pieces))
                .collect(),
        )
    }
}

fn unstretched(
    quad: Quad,
    source: Rect,
    requested: (u32, u32),
    drawn: (u32, u32),
) -> Option<(Quad, Rect)> {
    let axis = |min: f32, max: f32, requested: u32, drawn: u32| {
        let (from, to) = (min * requested as f32, max * requested as f32);
        let end = to.min(drawn as f32);
        (end > from && drawn > 0).then(|| {
            (
                from / drawn as f32,
                end / drawn as f32,
                (end - from) / (to - from),
            )
        })
    };
    let (left, right, across) = axis(source.min.x, source.max.x, requested.0, drawn.0)?;
    let (top, bottom, down) = axis(source.min.y, source.max.y, requested.1, drawn.1)?;
    let [origin, along, _, below] = quad.corners;
    let at = |x: f32, y: f32| origin + (along - origin) * x + (below - origin) * y;
    let corners = [
        at(0.0, 0.0),
        at(across, 0.0),
        at(across, down),
        at(0.0, down),
    ];
    Some((
        Quad {
            rect: Rect::from_points(&corners),
            corners,
            opacity: quad.opacity,
        },
        Rect::from_min_max(pos2(left, top), pos2(right, bottom)),
    ))
}

pub(super) fn damaged_pieces(
    rect: SurfaceRect,
    placement: ScreenPlacement,
    pieces: &[Piece],
) -> Vec<Rect> {
    let (x, y) = (placement.x as f32, placement.y as f32);
    let (width, height) = (placement.width as f32, placement.height as f32);
    let changed = Rect::from_min_max(
        pos2((rect.x as f32 - x) / width, (rect.y as f32 - y) / height),
        pos2(
            (rect.x as f32 + rect.width as f32 - x) / width,
            (rect.y as f32 + rect.height as f32 - y) / height,
        ),
    );
    pieces
        .iter()
        .filter_map(|piece| {
            let hit = changed.intersect(piece.source);
            if !hit.is_positive() || !piece.source.is_positive() {
                return None;
            }
            let local = |point: Pos2| {
                pos2(
                    piece.local.min.x
                        + (point.x - piece.source.min.x) / piece.source.width()
                            * piece.local.width(),
                    piece.local.min.y
                        + (point.y - piece.source.min.y) / piece.source.height()
                            * piece.local.height(),
                )
            };
            Some(Rect::from_min_max(local(hit.min), local(hit.max)))
        })
        .collect()
}

struct Regions {
    buffer: wgpu::Buffer,
    group: wgpu::BindGroup,
    capacity: usize,
}

pub(crate) struct PluginDrawing {
    blits: RefCell<Vec<Blit>>,
    regions: RefCell<Option<Regions>>,
    placed: RefCell<Vec<Option<u32>>>,
}

impl PluginDrawing {
    pub(crate) fn new(blits: Vec<Blit>) -> Self {
        Self {
            blits: RefCell::new(blits),
            regions: RefCell::new(None),
            placed: RefCell::new(Vec::new()),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Piece {
    pub(crate) local: Rect,
    pub(crate) source: Rect,
}

pub(crate) struct RegionDrawing {
    #[cfg(target_arch = "wasm32")]
    id: u64,
    template: Blit,
    pieces: Vec<Piece>,
    held: Option<Rect>,
    rotation: f32,
    drawing: PluginDrawing,
}

impl RegionDrawing {
    pub(super) fn new(
        template: Blit,
        pieces: Vec<Piece>,
        held: Option<Rect>,
        rotation: f32,
    ) -> Self {
        Self {
            #[cfg(target_arch = "wasm32")]
            id: next_drawing(),
            template,
            pieces,
            held,
            rotation,
            drawing: PluginDrawing::new(Vec::new()),
        }
    }

    fn blits(&self, at: &DrawAt) -> Vec<Blit> {
        let scale = at.pixels_per_point.max(f32::EPSILON);
        let laid = Rect::from_min_max(
            pos2(at.rect[0] / scale, at.rect[1] / scale),
            pos2(at.rect[2] / scale, at.rect[3] / scale),
        );
        let rect = self.held.unwrap_or(laid);
        let center = rect.center();
        let (sin, cos) = self.rotation.sin_cos();
        let turn = |point: Pos2| {
            let offset = point - center;
            center
                + vec2(
                    offset.x * cos - offset.y * sin,
                    offset.x * sin + offset.y * cos,
                )
        };
        let at_fraction = |x: f32, y: f32| {
            turn(pos2(
                rect.min.x + rect.width() * x,
                rect.min.y + rect.height() * y,
            ))
        };
        self.pieces
            .iter()
            .map(|piece| {
                let local = piece.local;
                let corners = [
                    at_fraction(local.min.x, local.min.y),
                    at_fraction(local.max.x, local.min.y),
                    at_fraction(local.max.x, local.max.y),
                    at_fraction(local.min.x, local.max.y),
                ];
                Blit {
                    quad: Quad {
                        rect: Rect::from_points(&corners),
                        corners,
                        opacity: self.template.quad.opacity,
                    },
                    source: piece.source,
                    ..self.template.clone()
                }
            })
            .collect()
    }
}

#[cfg(target_arch = "wasm32")]
thread_local! {
    static SHOWN: RefCell<std::collections::HashMap<u64, (u64, Vec<Blit>)>> =
        RefCell::new(std::collections::HashMap::new());
    static PREPARED: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

#[cfg(target_arch = "wasm32")]
fn next_drawing() -> u64 {
    thread_local! {
        static NEXT: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    }
    NEXT.with(|next| {
        next.set(next.get() + 1);
        next.get()
    })
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn shown() -> Vec<Blit> {
    SHOWN.with(|shown| {
        let shown = shown.borrow();
        let mut ordered: Vec<&(u64, Vec<Blit>)> = shown.values().collect();
        ordered.sort_by_key(|(order, _)| *order);
        ordered
            .into_iter()
            .flat_map(|(_, blits)| blits.iter().cloned())
            .collect()
    })
}

#[cfg(target_arch = "wasm32")]
fn record_shown(id: u64, blits: &[Blit]) {
    let order = PREPARED.with(|prepared| {
        prepared.set(prepared.get() + 1);
        prepared.get()
    });
    let moved = SHOWN.with(|shown| {
        let mut shown = shown.borrow_mut();
        let moved = shown
            .get(&id)
            .is_none_or(|(_, held)| held.as_slice() != blits);
        shown.insert(id, (order, blits.to_vec()));
        moved
    });
    if moved {
        crate::host::wake();
    }
}

#[cfg(target_arch = "wasm32")]
impl Drop for RegionDrawing {
    fn drop(&mut self) {
        SHOWN.with(|shown| shown.borrow_mut().remove(&self.id));
        crate::host::wake();
    }
}

impl beui::Draw for RegionDrawing {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        at: DrawAt,
    ) {
        let blits = self.blits(&at);
        #[cfg(target_arch = "wasm32")]
        record_shown(self.id, &blits);
        *self.drawing.blits.borrow_mut() = blits;
        self.drawing.prepare(device, queue, encoder, at);
    }

    fn paint(&self, pass: &mut wgpu::RenderPass<'_>, at: DrawAt) {
        self.drawing.paint(pass, at);
    }
}

impl beui::Draw for PluginDrawing {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _encoder: &mut wgpu::CommandEncoder,
        at: DrawAt,
    ) {
        let blits = self.blits.borrow();
        PRESENTER.with(|presenter| {
            let mut presenter = presenter.borrow_mut();
            let Some(presenter) = presenter.as_mut() else {
                for blit in blits.iter() {
                    blit.status.set(PresenterState::Unsupported(
                        "The active renderer has no plugin surface presenter.".to_owned(),
                    ));
                }
                return;
            };
            let mut regions = self.regions.borrow_mut();
            let needed = blits.len().max(1);
            if regions
                .as_ref()
                .is_none_or(|regions| regions.capacity < needed)
            {
                let capacity = needed.next_power_of_two();
                let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("plugin surface regions"),
                    size: u64::from(presenter.regions.stride) * capacity as u64,
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("plugin surface regions"),
                    layout: &presenter.regions.layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: &buffer,
                            offset: 0,
                            size: wgpu::BufferSize::new(REGION_BYTES),
                        }),
                    }],
                });
                *regions = Some(Regions {
                    buffer,
                    group,
                    capacity,
                });
            }
            let regions = regions.as_ref().expect("the regions were just created");
            let mut placed = self.placed.borrow_mut();
            placed.clear();
            for (index, blit) in blits.iter().enumerate() {
                let (frames, region) = {
                    let mut shared = blit.shared.borrow_mut();
                    let frames = shared.take_frames();
                    let region = shared.layout.placement(blit.screen).and_then(|placement| {
                        let size = (placement.width, placement.height);
                        let (quad, source) = match (blit.drawn, blit.requested) {
                            (None, Some(requested)) if requested != size => {
                                unstretched(blit.quad, blit.source, requested, size)?
                            }
                            _ => (blit.quad, blit.source),
                        };
                        Region::of(&shared.layout, blit.screen, quad, source)
                            .filter(|_| blit.drawn.is_none_or(|drawn| drawn == size))
                    });
                    (frames, region)
                };
                let mut failure = None;
                for frame in &frames {
                    let applied = presenter
                        .replace(device, blit.surface, frame)
                        .and_then(|()| presenter.prepare(queue, blit.surface, frame));
                    if let Err(error) = applied {
                        failure = Some(error);
                    }
                }
                match failure {
                    Some(error) => blit.status.set(PresenterState::Failed(error)),
                    None if presenter.platform.is_some() => {
                        blit.status.set(PresenterState::Presenting);
                    }
                    None => blit
                        .status
                        .set(PresenterState::Unsupported(UNSUPPORTED.to_owned())),
                }
                placed.push(region.map(|region| {
                    let offset = presenter.regions.stride * index as u32;
                    queue.write_buffer(
                        &regions.buffer,
                        u64::from(offset),
                        bytemuck::cast_slice(&region.values(&at)),
                    );
                    offset
                }));
            }
        });
    }

    fn paint(&self, pass: &mut wgpu::RenderPass<'_>, _at: DrawAt) {
        PRESENTER.with(|presenter| {
            let presenter = presenter.borrow();
            let Some(presenter) = presenter.as_ref() else {
                return;
            };
            let Some(platform) = &presenter.platform else {
                return;
            };
            let regions = self.regions.borrow();
            let Some(regions) = regions.as_ref() else {
                return;
            };
            let placed = self.placed.borrow();
            let blits = self.blits.borrow();
            for (blit, offset) in blits.iter().zip(placed.iter()) {
                if let Some(offset) = offset {
                    platform.paint(pass, blit.surface, &regions.group, *offset);
                }
            }
        });
    }
}

thread_local! {
    static PRESENTER: RefCell<Option<Presenter>> = const { RefCell::new(None) };
}

struct Presenter {
    regions: RegionLayout,
    platform: Option<PlatformPresenter>,
}

pub(super) fn install(setup: &beui::Setup) -> Availability {
    let gpu = setup
        .get::<beui::GpuSetup>()
        .expect("beui's runner provides the GPU it draws with");
    let regions = RegionLayout::new(&gpu.device);
    let platform = build_presenter(&gpu.device, &gpu.queue, &regions, gpu.format);
    let availability = Availability(platform.as_ref().map(|_| ()).map_err(Clone::clone));
    PRESENTER.with(|presenter| {
        *presenter.borrow_mut() = Some(Presenter {
            regions,
            platform: platform.ok(),
        });
    });
    availability
}

pub(super) fn release(surface: u32, status: &PresenterStatus) {
    PRESENTER.with(|presenter| {
        if let Some(platform) = presenter
            .borrow_mut()
            .as_mut()
            .and_then(|presenter| presenter.platform.as_mut())
        {
            platform.release(surface);
        }
    });
    status.set(PresenterState::Released);
}

impl Presenter {
    fn replace(
        &mut self,
        device: &wgpu::Device,
        surface: u32,
        frame: &Frame,
    ) -> Result<(), String> {
        match &mut self.platform {
            Some(presenter) => presenter.replace(device, surface, frame),
            None => Err(UNSUPPORTED.to_owned()),
        }
    }

    fn prepare(&mut self, queue: &wgpu::Queue, surface: u32, frame: &Frame) -> Result<(), String> {
        match &mut self.platform {
            Some(presenter) => presenter.prepare(queue, surface, frame),
            None => Err(UNSUPPORTED.to_owned()),
        }
    }
}

const UNSUPPORTED: &str = "This build has no presenter for that plugin surface.";

#[cfg(test)]
mod tests;
