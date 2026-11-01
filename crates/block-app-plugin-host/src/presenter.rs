use std::{
    cell::RefCell,
    collections::VecDeque,
    rc::Rc,
    sync::{Arc, Mutex},
};

use beui::native::painting::{Painting, Paints, Placement};
use beui::{
    Pos2, Rect, Rotation, Vec2,
    native::{DrawAt, texture_quad},
    pos2, vec2,
};
use block_plugin_api::{ScreenDamage, ScreenId, ScreenLayout, ScreenPlacement, SurfaceRect};

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
        slot: u32,
        frame: &Self::Frame,
    ) -> Result<(), String>;

    fn pipelines(&self) -> &texture_quad::Pipelines;

    fn texture(&self, slot: u32, surface: u32) -> Option<&texture_quad::Bound>;

    fn shown(&self, quad: texture_quad::Quad) -> texture_quad::Quad {
        quad
    }

    fn size(&self, _slot: u32, _surface: u32) -> Option<(u32, u32)> {
        None
    }

    fn retain(&mut self, slot: u32, surfaces: &[u32]);

    fn release(&mut self, slot: u32);
}

const MAX_PENDING_FRAMES: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quad {
    pub rect: Rect,
    pub rotation: f32,
    pub opacity: f32,
    pub corner_radius: f32,
}

impl Quad {
    pub fn upright(rect: Rect) -> Self {
        Self {
            rect,
            rotation: 0.0,
            opacity: 1.0,
            corner_radius: 0.0,
        }
    }

    fn turning(&self) -> Rotation {
        Rotation::new(self.rect.center(), self.rotation)
    }

    pub fn part(self, fraction: Rect) -> Self {
        let at = |point: Pos2| {
            pos2(
                self.rect.min.x + self.rect.width() * point.x,
                self.rect.min.y + self.rect.height() * point.y,
            )
        };
        let part = Rect::from_min_max(at(fraction.min), at(fraction.max));
        Self {
            rect: part.translate(self.turning().apply(part.center()) - part.center()),
            ..self
        }
    }

    pub fn corners(&self) -> [Pos2; 4] {
        [
            self.rect.left_top(),
            self.rect.right_top(),
            self.rect.right_bottom(),
            self.rect.left_bottom(),
        ]
        .map(|corner| self.turning().apply(corner))
    }

    pub fn bounds(&self) -> Rect {
        self.turning().bounds(self.rect)
    }

    fn placed(&self, source: Rect, at: &DrawAt) -> texture_quad::Quad {
        let scale = at.pixels_per_point;
        texture_quad::Quad {
            rect: [
                self.rect.min.x * scale,
                self.rect.min.y * scale,
                self.rect.max.x * scale,
                self.rect.max.y * scale,
            ],
            uv: [source.min.x, source.min.y, source.max.x, source.max.y],
            clip: at.clip,
            rotation: self.rotation,
            opacity: self.opacity.clamp(0.0, 1.0),
            corner_radius: self.corner_radius.max(0.0) * scale,
            srgb_encoded: true,
            ..texture_quad::Quad::default()
        }
    }
}

pub(super) fn shown_part(rect: Rect, clip: Rect) -> Option<Rect> {
    let shown = rect.intersect(clip);
    let fraction = |point: Pos2| {
        pos2(
            (point.x - rect.min.x) / rect.width(),
            (point.y - rect.min.y) / rect.height(),
        )
    };
    (rect.is_positive() && shown.is_positive())
        .then(|| Rect::from_min_max(fraction(shown.min), fraction(shown.max)))
}

#[derive(Default)]
pub(super) struct Shared {
    pub(super) layout: ScreenLayout,
    pub(super) frames: Vec<Frame>,
    pub(super) paintings: stable_hash::HashMap<ScreenId, Painting>,
    pub(super) overflows: stable_hash::HashMap<ScreenId, Vec<String>>,
    pub(super) published: Published,
}

impl Shared {
    pub(super) fn publish(&mut self, layout: &ScreenLayout, frame: Option<Frame>) {
        self.layout.clone_from(layout);
        let Some(frame) = frame else {
            return;
        };
        self.published.record(frame.damage());
        if self.frames.len() >= MAX_PENDING_FRAMES {
            self.frames.remove(0);
        }
        self.frames.push(frame);
    }

    fn take_frames(&mut self) -> Vec<Frame> {
        std::mem::take(&mut self.frames)
    }
}

const REMEMBERED_FRAMES: usize = 16;

#[derive(Default)]
pub(super) struct Published {
    count: u64,
    recent: VecDeque<Option<Vec<ScreenDamage>>>,
}

impl Published {
    pub(super) fn record(&mut self, damage: Option<&[ScreenDamage]>) {
        self.count += 1;
        if self.recent.len() == REMEMBERED_FRAMES {
            self.recent.pop_front();
        }
        self.recent.push_back(damage.map(<[ScreenDamage]>::to_vec));
    }

    pub(super) fn count(&self) -> u64 {
        self.count
    }

    pub(super) fn since(&self, seen: u64) -> Option<Vec<ScreenDamage>> {
        let missed = usize::try_from(self.count.checked_sub(seen)?).ok()?;
        let skipped = self.recent.len().checked_sub(missed)?;
        let mut damage = Vec::new();
        for frame in self.recent.iter().skip(skipped) {
            damage.extend_from_slice(frame.as_deref()?);
        }
        Some(damage)
    }
}

#[derive(Clone)]
pub struct Blit {
    pub(super) surface: u32,
    pub(super) status: PresenterStatus,
    pub(super) shared: Rc<RefCell<Shared>>,
    pub(super) screen: ScreenId,
    pub(super) quad: Quad,
    pub(super) source: Rect,
    pub(super) drawn: Option<(u32, u32)>,
    pub(super) requested: Option<(u32, u32)>,
    pub(super) placed: Option<[u32; 3]>,
    pub(super) layer: Option<AnchoredSource>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct AnchoredSource {
    pub(super) surface: u32,
    pub(super) pixels: Rect,
    pub(super) bounds: Rect,
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
            && self.layer == other.layer
    }
}

impl Blit {
    pub fn rect(&self) -> Rect {
        self.quad.bounds()
    }

    pub(super) fn shown_at(&self, painted: (u32, u32)) -> Option<(Quad, Rect)> {
        if self.drawn.is_some_and(|drawn| drawn != painted) {
            return None;
        }
        match (self.drawn, self.requested) {
            (None, Some(requested)) if requested != painted => {
                unstretched(self.quad, self.source, requested, painted)
            }
            _ => Some((self.quad, self.source)),
        }
    }

    pub(super) fn damage(&self, pieces: &[Piece], seen: u64) -> Option<Vec<Rect>> {
        let shared = self.shared.borrow();
        let damage = shared.published.since(seen)?;
        if damage.is_empty() {
            return Some(Vec::new());
        }
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
                .filter(|damage| damage.screen == self.screen)
                .flat_map(|damage| damaged_pieces(damage.rect, *placement, pieces))
                .collect(),
        )
    }
}

pub(super) fn unstretched(
    quad: Quad,
    source: Rect,
    requested: (u32, u32),
    drawn: (u32, u32),
) -> Option<(Quad, Rect)> {
    if drawn.0 == 0 || drawn.1 == 0 {
        return None;
    }
    let (across, down) = (
        requested.0 as f32 / drawn.0 as f32,
        requested.1 as f32 / drawn.1 as f32,
    );
    within_texture(
        quad,
        Rect::from_min_max(
            pos2(source.min.x * across, source.min.y * down),
            pos2(source.max.x * across, source.max.y * down),
        ),
    )
}

pub(super) fn within_texture(quad: Quad, source: Rect) -> Option<(Quad, Rect)> {
    let axis = |min: f32, max: f32| {
        let end = max.min(1.0);
        (end > min).then(|| (end, (end - min) / (max - min)))
    };
    let (right, across) = axis(source.min.x, source.max.x)?;
    let (bottom, down) = axis(source.min.y, source.max.y)?;
    Some((
        quad.part(Rect::from_min_max(Pos2::ZERO, pos2(across, down))),
        Rect::from_min_max(source.min, pos2(right, bottom)),
    ))
}

pub(super) fn damaged_pieces(
    rect: SurfaceRect,
    placement: ScreenPlacement,
    pieces: &[Piece],
) -> Vec<Rect> {
    let (width, height) = (placement.width as f32, placement.height as f32);
    let changed = Rect::from_min_max(
        pos2(rect.x as f32 / width, rect.y as f32 / height),
        pos2(
            (rect.x as f32 + rect.width as f32) / width,
            (rect.y as f32 + rect.height as f32) / height,
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

pub struct PluginDrawing {
    blits: RefCell<Vec<Blit>>,
    quads: texture_quad::Quads,
}

impl PluginDrawing {
    pub fn new(blits: Vec<Blit>) -> Self {
        Self {
            blits: RefCell::new(blits),
            quads: texture_quad::Quads::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Piece {
    pub local: Rect,
    pub source: Rect,
}

pub struct ViewDrawing {
    #[cfg(target_arch = "wasm32")]
    id: u64,
    template: Blit,
    made_for: Option<Vec2>,
    pieces: Pieces,
    held: Option<Rect>,
    rotation: f32,
    drawing: PluginDrawing,
}

pub type Pieces = Rc<RefCell<Vec<Piece>>>;

impl ViewDrawing {
    pub(super) fn new(
        template: Blit,
        made_for: Option<Vec2>,
        pieces: Pieces,
        held: Option<Rect>,
        rotation: f32,
    ) -> Self {
        Self {
            #[cfg(target_arch = "wasm32")]
            id: next_drawing(),
            template,
            made_for,
            pieces,
            held,
            rotation,
            drawing: PluginDrawing::new(Vec::new()),
        }
    }

    fn blits(&self, at: &DrawAt) -> Vec<Blit> {
        let scale = at.pixels_per_point.max(f32::EPSILON);
        self.blits_in(Rect::from_min_max(
            pos2(at.rect[0] / scale, at.rect[1] / scale),
            pos2(at.rect[2] / scale, at.rect[3] / scale),
        ))
    }

    fn blits_in(&self, laid: Rect) -> Vec<Blit> {
        let rect = self.held.unwrap_or(laid);
        let requested = match (self.template.requested, self.made_for) {
            (Some((width, height)), Some(made)) if made.x > 0.0 && made.y > 0.0 => Some((
                (width as f32 * rect.width() / made.x).round() as u32,
                (height as f32 * rect.height() / made.y).round() as u32,
            )),
            (requested, _) => requested,
        };
        let view = Quad {
            rect,
            rotation: self.rotation,
            ..self.template.quad
        };
        self.pieces
            .borrow()
            .iter()
            .map(|piece| Blit {
                quad: view.part(piece.local),
                source: piece.source,
                requested,
                ..self.template.clone()
            })
            .collect()
    }
}

pub type AnchoredBlits = Rc<RefCell<Vec<Blit>>>;

pub struct AnchoredDrawing {
    #[cfg(target_arch = "wasm32")]
    id: u64,
    blits: AnchoredBlits,
    drawing: PluginDrawing,
}

impl AnchoredDrawing {
    pub(super) fn new(blits: AnchoredBlits) -> Self {
        Self {
            #[cfg(target_arch = "wasm32")]
            id: next_drawing(),
            blits,
            drawing: PluginDrawing::new(Vec::new()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
impl Drop for AnchoredDrawing {
    fn drop(&mut self) {
        SHOWN.with(|shown| shown.borrow_mut().remove(&self.id));
        block_app_base::host::wake();
    }
}

impl beui::native::Draw for AnchoredDrawing {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        at: DrawAt,
    ) {
        let scale = at.pixels_per_point.max(f32::EPSILON);
        let offset = vec2(at.rect[0] / scale, at.rect[1] / scale);
        let blits: Vec<Blit> = self
            .blits
            .borrow()
            .iter()
            .map(|blit| Blit {
                quad: Quad {
                    rect: blit.quad.rect.translate(offset),
                    ..blit.quad
                },
                layer: blit.layer.map(|layer| AnchoredSource {
                    bounds: layer.bounds.translate(offset),
                    ..layer
                }),
                ..blit.clone()
            })
            .collect();
        #[cfg(target_arch = "wasm32")]
        record_shown(self.id, &blits);
        *self.drawing.blits.borrow_mut() = blits;
        self.drawing.prepare(device, queue, encoder, at);
    }

    fn paint(&self, pass: &mut wgpu::RenderPass<'_>, at: DrawAt) {
        self.drawing.paint(pass, at);
    }
}

impl Paints for AnchoredDrawing {
    fn overflows(&self, _rect: Rect) -> Vec<String> {
        Vec::new()
    }

    fn painting(&self, _rect: Rect) -> Option<Painting> {
        let mut primitives = Vec::new();
        let mut textures = std::collections::BTreeMap::new();
        for blit in self.blits.borrow().iter() {
            let Some(layer) = blit.layer else {
                continue;
            };
            let shared = blit.shared.borrow();
            let Some(painting) = shared.paintings.get(&blit.screen) else {
                continue;
            };
            let [width, height] = painting.frame.size.map(|side| side.max(1) as f32);
            let pixels = layer.pixels;
            let Some((quad, source)) = within_texture(
                blit.quad,
                Rect::from_min_max(
                    pos2(pixels.min.x / width, pixels.min.y / height),
                    pos2(pixels.max.x / width, pixels.max.y / height),
                ),
            ) else {
                continue;
            };
            primitives.extend(painting.placed(Placement {
                source: [source.min.x, source.min.y, source.max.x, source.max.y],
                corners: quad.corners().map(|corner| [corner.x, corner.y]),
                clip: {
                    let bounds = quad.bounds();
                    [bounds.min.x, bounds.min.y, bounds.max.x, bounds.max.y]
                },
                corner_radius: 0.0,
            }));
            for (key, pixels) in &painting.textures {
                textures.entry(*key).or_insert_with(|| pixels.clone());
            }
        }
        Some(Painting {
            frame: beui::native::painting::Frame {
                size: [1, 1],
                pixels_per_point: 1.0,
                background: [0; 4],
                primitives,
            },
            textures,
        })
    }
}

#[cfg(target_arch = "wasm32")]
thread_local! {
    static SHOWN: RefCell<stable_hash::HashMap<u64, (u64, Vec<Blit>)>> =
        RefCell::new(stable_hash::HashMap::default());
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
pub fn shown() -> Vec<Blit> {
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
        block_app_base::host::wake();
    }
}

#[cfg(target_arch = "wasm32")]
impl Drop for ViewDrawing {
    fn drop(&mut self) {
        SHOWN.with(|shown| shown.borrow_mut().remove(&self.id));
        block_app_base::host::wake();
    }
}

impl beui::native::Draw for ViewDrawing {
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

impl Paints for ViewDrawing {
    fn overflows(&self, rect: Rect) -> Vec<String> {
        let mut found = Vec::new();
        for blit in self.blits_in(rect) {
            if let Some(overflows) = blit.shared.borrow().overflows.get(&blit.screen) {
                found.extend_from_slice(overflows);
            }
        }
        found
    }

    fn painting(&self, rect: Rect) -> Option<Painting> {
        let mut primitives = Vec::new();
        let mut textures = std::collections::BTreeMap::new();
        let mut found = false;
        for blit in self.blits_in(rect) {
            let shared = blit.shared.borrow();
            let Some(placement) = shared.layout.placement(blit.screen) else {
                continue;
            };
            let placed = blit.shown_at((placement.width, placement.height));
            let (Some((quad, source)), Some(screen)) = (placed, shared.paintings.get(&blit.screen))
            else {
                continue;
            };
            let screen = super::previews::painting(screen.clone());
            found = true;
            primitives.extend(screen.placed(Placement {
                source: [source.min.x, source.min.y, source.max.x, source.max.y],
                corners: quad.corners().map(|corner| [corner.x, corner.y]),
                clip: {
                    let bounds = quad.bounds();
                    [bounds.min.x, bounds.min.y, bounds.max.x, bounds.max.y]
                },
                corner_radius: quad.corner_radius,
            }));
            for (key, pixels) in &screen.textures {
                textures.entry(*key).or_insert_with(|| pixels.clone());
            }
        }
        (found || self.pieces.borrow().is_empty()).then_some(Painting {
            frame: beui::native::painting::Frame {
                size: [1, 1],
                pixels_per_point: 1.0,
                background: [0; 4],
                primitives,
            },
            textures,
        })
    }
}

impl beui::native::Draw for PluginDrawing {
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
            let Some(platform) = &mut presenter.platform else {
                for blit in blits.iter() {
                    blit.shared.borrow_mut().take_frames();
                    blit.status
                        .set(PresenterState::Unsupported(UNSUPPORTED.to_owned()));
                }
                return;
            };
            let mut placed = Vec::new();
            for blit in blits.iter() {
                let (frames, region, surfaces) = {
                    let mut shared = blit.shared.borrow_mut();
                    let frames = shared.take_frames();
                    let laid = |placement: &ScreenPlacement| {
                        let (quad, source) = blit.shown_at((placement.width, placement.height))?;
                        Some((placement.surface, quad, source))
                    };
                    let region = match blit.layer {
                        Some(_) => None,
                        None => shared.layout.placement(blit.screen).and_then(laid),
                    };
                    let surfaces: Vec<u32> = shared
                        .layout
                        .screens
                        .iter()
                        .map(|placement| placement.surface)
                        .collect();
                    (frames, region, surfaces)
                };
                let mut failure = None;
                for frame in &frames {
                    if let Err(error) = platform.replace(device, blit.surface, frame) {
                        failure = Some(error);
                    }
                }
                if !frames.is_empty() {
                    platform.retain(blit.surface, &surfaces);
                }
                let region =
                    match blit.layer {
                        Some(layer) => platform.size(blit.surface, layer.surface).and_then(
                            |(width, height)| {
                                let (width, height) = (width as f32, height as f32);
                                let source = Rect::from_min_max(
                                    pos2(layer.pixels.min.x / width, layer.pixels.min.y / height),
                                    pos2(layer.pixels.max.x / width, layer.pixels.max.y / height),
                                );
                                let (quad, source) = (width > 0.0 && height > 0.0)
                                    .then(|| within_texture(blit.quad, source))??;
                                Some((layer.surface, quad, source))
                            },
                        ),
                        None => region,
                    };
                blit.status.set(match failure {
                    Some(error) => PresenterState::Failed(error),
                    None => PresenterState::Presenting,
                });
                if let Some((surface, quad, source)) = region {
                    placed.push((
                        blit.surface,
                        surface,
                        platform.shown(quad.placed(source, &at)),
                    ));
                }
            }
            let platform = &*platform;
            self.quads.prepare(
                platform.pipelines(),
                queue,
                at.format,
                [at.screen.x, at.screen.y],
                placed.into_iter().filter_map(|(slot, surface, quad)| {
                    Some((platform.texture(slot, surface)?, quad))
                }),
            );
        });
    }

    fn paint(&self, pass: &mut wgpu::RenderPass<'_>, _at: DrawAt) {
        self.quads.paint(pass);
    }
}

thread_local! {
    static PRESENTER: RefCell<Option<Presenter>> = const { RefCell::new(None) };
}

struct Presenter {
    platform: Option<PlatformPresenter>,
}

pub(super) fn install(setup: &beui::native::Setup) -> Availability {
    let Some(gpu) = setup.get::<beui_host::native::GpuSetup>() else {
        return Availability(Ok(()));
    };
    let platform = build_presenter(&gpu.device, &gpu.queue);
    let availability = Availability(platform.as_ref().map(|_| ()).map_err(Clone::clone));
    PRESENTER.with(|presenter| {
        *presenter.borrow_mut() = Some(Presenter {
            platform: platform.ok(),
        });
    });
    availability
}

pub(super) fn rendered() -> bool {
    PRESENTER.with(|presenter| presenter.borrow().is_some())
}

pub(super) fn stop() {
    let presenter = PRESENTER.with(|presenter| presenter.borrow_mut().take());
    drop(presenter);
}

pub(super) fn release(slot: u32, status: &PresenterStatus) {
    PRESENTER.with(|presenter| {
        if let Some(platform) = presenter
            .borrow_mut()
            .as_mut()
            .and_then(|presenter| presenter.platform.as_mut())
        {
            platform.release(slot);
        }
    });
    status.set(PresenterState::Released);
}

const UNSUPPORTED: &str = "This build has no presenter for that plugin surface.";

#[cfg(test)]
mod tests;
