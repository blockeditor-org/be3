use std::{
    cell::RefCell,
    collections::VecDeque,
    rc::Rc,
    sync::{Arc, Mutex},
};

use be_sdk_api::{ScreenDamage, ScreenId, ScreenLayout, ScreenPlacement, SurfaceRect};
use beui::native::painting::{Painting, Paints, Placement};
use beui::{
    Pos2, Rect, Rotation, Vec2,
    native::{DrawAt, texture_quad},
    pos2, vec2,
};

use be_lib_layers::{Layer, Surface};

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

    pub(super) fn placed(&self, source: Rect, at: &DrawAt) -> texture_quad::Quad {
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
    /// The last frames the runtime keeps of views the app's layout left out (`runtime::Kept`).
    pub(super) kept: Vec<ScreenPlacement>,
    pub(super) frames: Vec<Frame>,
    pub(super) paintings: be_lib_stable_hash::HashMap<ScreenId, Painting>,
    pub(super) overflows: be_lib_stable_hash::HashMap<ScreenId, Vec<String>>,
    pub(super) published: Published,
    /// The placement each surface was last painted at (`presented-frame.painted`), so its size
    /// and raster are those of what it holds now, not of what the layout has asked for since.
    /// Kept per surface and pruned against the layout each `publish` brings.
    pub(super) painted: be_lib_stable_hash::HashMap<u32, ScreenPlacement>,
}

/// What a surface holds: the size of its texture or painting and the raster it was painted at.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Painted {
    pub(super) size: (u32, u32),
    pub(super) raster: f32,
}

impl Shared {
    pub(super) fn publish(
        &mut self,
        layout: &ScreenLayout,
        kept: Vec<ScreenPlacement>,
        frame: Option<Frame>,
    ) {
        self.layout.clone_from(layout);
        self.kept = kept;
        // Pruned only here, against the layout just published: a surface's first present can
        // be heard before the layout that brought it is published (both arrive in one pump).
        // A present is never held behind a layout older than its own (`Runtime::received` holds
        // a frame with the layout it follows), so the layout published here is never older than
        // a surface painted in it.
        let surfaces = self.surfaces();
        self.painted.retain(|surface, _| surfaces.contains(surface));
        let Some(frame) = frame else {
            return;
        };
        self.published.record(frame.damage());
        if self.frames.len() >= MAX_PENDING_FRAMES {
            self.frames.remove(0);
        }
        self.frames.push(frame);
    }

    pub(super) fn paint(&mut self, painted: &[ScreenPlacement]) {
        for placement in painted {
            self.painted.insert(placement.surface, *placement);
        }
    }

    /// What the surface `placement` names holds: `size` where the caller knows the size of what
    /// it draws (the texture, the painting), else the size it was last painted at; the raster
    /// it was painted at, or the layout's before any present reported one. `None` where nothing
    /// was painted there yet.
    pub(super) fn holds(
        &self,
        placement: &ScreenPlacement,
        size: Option<(u32, u32)>,
    ) -> Option<Painted> {
        let painted = self.painted.get(&placement.surface);
        Some(Painted {
            size: size.or_else(|| painted.map(|painted| (painted.width, painted.height)))?,
            raster: painted.unwrap_or(placement).raster(),
        })
    }

    fn take_frames(&mut self) -> Vec<Frame> {
        std::mem::take(&mut self.frames)
    }

    /// The placement a screen is drawn from: its kept last frame while it has one, else the
    /// layout's.
    pub(super) fn placement(&self, screen: ScreenId) -> Option<&ScreenPlacement> {
        self.kept
            .iter()
            .find(|placement| placement.screen == screen)
            .or_else(|| self.layout.placement(screen))
    }

    fn surfaces(&self) -> Vec<u32> {
        self.layout
            .screens
            .iter()
            .chain(&self.kept)
            .map(|placement| placement.surface)
            .collect()
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
    /// The zoom the host shows the view at (`Placed::zoom`): a painting at the raster it was
    /// asked for covers its pixels over the raster times this, in the host's points.
    pub(super) zoom: f32,
    /// A block preview, which is a picture: fitted to each place it is shown at (a preview shown
    /// in two places is painted for the larger), where an editor's view never is.
    pub(super) picture: bool,
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
            && self.zoom == other.zoom
            && self.picture == other.picture
            && self.placed == other.placed
            && self.layer == other.layer
    }
}

impl Blit {
    pub fn rect(&self) -> Rect {
        self.quad.bounds()
    }

    /// Where a painting of `painted` pixels, at `placement`'s raster, shows for this blit, and
    /// which part of it: always at its own size, from the view's top left, cropped where the view
    /// is smaller and leaving the rest of the view where it is larger to what is under it
    /// (`unscaled`). Every path that draws an app's painting into its view, native, web or a
    /// paint snapshot, goes through here. `painted` is the size of what is drawn (the texture,
    /// the painting), never the size the layout asked for, since a layout commit can land
    /// before the present that brings textures of its size.
    /// The layer that shows this blit of `placement`'s surface, natively and as the hole the web
    /// punches for its canvas: at the size the renderer says the texture is (`texture`; the web
    /// keeps no textures, so it is the size the worker last painted that surface at). `piece`
    /// tells apart the layers of one view's pieces.
    pub(super) fn screen_layer(
        &self,
        placement: &ScreenPlacement,
        texture: Option<(u32, u32)>,
        piece: usize,
    ) -> Option<Layer> {
        let painted = self.shared.borrow().holds(placement, texture)?;
        crate::layers::screen(
            crate::layers::id(self.surface, self.screen.0, false, piece),
            Surface::Screen {
                instance: self.surface,
                surface: placement.surface,
            },
            self.shown_at(painted)?,
            painted,
        )
    }

    /// The layer that shows this blit's part of an anchored surface whose texture is `size`.
    pub(super) fn anchored_layer(
        &self,
        anchored: AnchoredSource,
        size: (u32, u32),
        piece: usize,
    ) -> Option<Layer> {
        let (width, height) = (size.0 as f32, size.1 as f32);
        if width <= 0.0 || height <= 0.0 {
            return None;
        }
        let source = Rect::from_min_max(
            pos2(
                anchored.pixels.min.x / width,
                anchored.pixels.min.y / height,
            ),
            pos2(
                anchored.pixels.max.x / width,
                anchored.pixels.max.y / height,
            ),
        );
        crate::layers::screen(
            crate::layers::id(self.surface, u64::from(anchored.surface), true, piece),
            Surface::Anchored {
                instance: self.surface,
                surface: anchored.surface,
            },
            within_texture(self.quad, source)?,
            Painted { size, raster: 1.0 },
        )
    }

    pub(super) fn shown_at(&self, painted: Painted) -> Option<(Quad, Rect)> {
        if self.picture {
            return (painted.size.0 > 0 && painted.size.1 > 0).then_some((self.quad, self.source));
        }
        unscaled(self.quad, self.source, painted.size, self.density(painted))
    }

    /// A painting's pixels per point of the host.
    fn density(&self, painted: Painted) -> f32 {
        painted.raster / self.zoom
    }

    pub(super) fn damage(&self, pieces: &[Piece], laid: Vec2, seen: u64) -> Option<Vec<Rect>> {
        let shared = self.shared.borrow();
        let damage = shared.published.since(seen)?;
        if damage.is_empty() {
            return Some(Vec::new());
        }
        let placement = shared.placement(self.screen)?;
        let painted = shared.holds(placement, None).unwrap_or(Painted {
            size: (placement.width, placement.height),
            raster: placement.raster(),
        });
        if painted.size != (placement.width, placement.height) {
            return None;
        }
        // Damage maps through the pieces' fractions, which only hold while the painting fills
        // the view as laid; one shown at another size is redrawn whole.
        let density = self.density(painted);
        let fills = pieces.iter().all(|piece| {
            let textured = vec2(
                piece.local.width() * laid.x / piece.source.width(),
                piece.local.height() * laid.y / piece.source.height(),
            ) * density;
            (textured.x - painted.size.0 as f32).abs() < 0.5
                && (textured.y - painted.size.1 as f32).abs() < 0.5
        });
        if !fills {
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

/// Places a painting of `painted` pixels, at `density` pixels per point, for a piece of a view
/// laid out at `quad` that shows `source` of the view's textured part: the painting keeps its own
/// size, anchored at the textured part's top left. Returns the part of `quad` it covers and the
/// part of the painting that shows there, or `None` where it covers none of the piece.
///
/// One point of the result's quad always shows `density` pixels of the painting along both axes,
/// so an app's frame is never stretched or squished into a view of another size, however late
/// its app answers: a view that grew shows the painting at its size with the rest left to what
/// is under it (the view's backdrop within its last placement, and beyond it what is behind the
/// view, until the app reports the new size), and one that shrank shows its top left.
pub(super) fn unscaled(
    quad: Quad,
    source: Rect,
    painted: (u32, u32),
    density: f32,
) -> Option<(Quad, Rect)> {
    if painted.0 == 0
        || painted.1 == 0
        || !(density.is_finite() && density > 0.0)
        || !source.is_positive()
        || !quad.rect.is_positive()
    {
        return None;
    }
    let painted = vec2(painted.0 as f32, painted.1 as f32);
    let rect = quad.rect;
    let laid = vec2(
        rect.width() / source.width(),
        rect.height() / source.height(),
    );
    let origin = rect.min - vec2(source.min.x * laid.x, source.min.y * laid.y);
    let extent = painted / density;
    let pixels = laid * density;
    if (pixels.x - painted.x).abs() < 1.0 && (pixels.y - painted.y).abs() < 1.0 {
        // The painting is the size the view is laid out at, in whole pixels, as it is whenever
        // its app has drawn that size: a view laid out at a fraction of a pixel is painted at
        // that size rounded (a 100.3-point view at 100 or 101 pixels). The piece then shows as it
        // was cut, with no float error.
        return Some((quad, source));
    }
    let covers = Rect::from_min_size(origin, extent);
    // An edge within a thousandth of a point of the piece's is the piece's: float error in
    // the mapping must not move a painting that fills its view by a fraction of a pixel.
    let snap = |value: f32, to: f32| match (value - to).abs() < 1e-3 {
        true => to,
        false => value,
    };
    let covers = Rect::from_min_max(
        pos2(
            snap(covers.min.x, rect.min.x),
            snap(covers.min.y, rect.min.y),
        ),
        pos2(
            snap(covers.max.x, rect.max.x),
            snap(covers.max.y, rect.max.y),
        ),
    );

    let shown = rect.intersect(covers);
    if !shown.is_positive() {
        return None;
    }
    let of_quad = |point: Pos2| {
        pos2(
            (point.x - rect.min.x) / rect.width(),
            (point.y - rect.min.y) / rect.height(),
        )
    };
    let of_painting = |point: Pos2| {
        pos2(
            (point.x - origin.x) * density / painted.x,
            (point.y - origin.y) * density / painted.y,
        )
    };
    Some((
        quad.part(Rect::from_min_max(of_quad(shown.min), of_quad(shown.max))),
        Rect::from_min_max(of_painting(shown.min), of_painting(shown.max)),
    ))
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

pub struct AppDrawing {
    blits: RefCell<Vec<Blit>>,
    quads: texture_quad::Quads,
}

impl AppDrawing {
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
    pieces: Pieces,
    held: Option<Rect>,
    rotation: f32,
    drawing: AppDrawing,
}

pub type Pieces = Rc<RefCell<Vec<Piece>>>;

impl ViewDrawing {
    pub(super) fn new(template: Blit, pieces: Pieces, held: Option<Rect>, rotation: f32) -> Self {
        Self {
            #[cfg(target_arch = "wasm32")]
            id: next_drawing(),
            template,
            pieces,
            held,
            rotation,
            drawing: AppDrawing::new(Vec::new()),
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
    drawing: AppDrawing,
}

impl AnchoredDrawing {
    pub(super) fn new(blits: AnchoredBlits) -> Self {
        Self {
            #[cfg(target_arch = "wasm32")]
            id: next_drawing(),
            blits,
            drawing: AppDrawing::new(Vec::new()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
impl Drop for AnchoredDrawing {
    fn drop(&mut self) {
        SHOWN.with(|shown| shown.borrow_mut().remove(&self.id));
        be_host_base::host::wake();
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
    static SHOWN: RefCell<be_lib_stable_hash::HashMap<u64, (u64, Vec<Blit>)>> =
        RefCell::new(be_lib_stable_hash::HashMap::default());
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
        be_host_base::host::wake();
    }
}

#[cfg(target_arch = "wasm32")]
impl Drop for ViewDrawing {
    fn drop(&mut self) {
        SHOWN.with(|shown| shown.borrow_mut().remove(&self.id));
        be_host_base::host::wake();
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
            let Some(placement) = shared.placement(blit.screen) else {
                continue;
            };
            let Some(screen) = shared.paintings.get(&blit.screen) else {
                continue;
            };
            // The view has a painting, so it is drawn even where the painting does not reach a
            // piece (a view grown before its app drew the new size): what is under the view shows
            // there, its backdrop within its last placement and beyond it what is behind it.
            found = true;
            let size = (screen.frame.size[0], screen.frame.size[1]);
            let Some((quad, source)) = shared
                .holds(placement, Some(size))
                .and_then(|painted| blit.shown_at(painted))
            else {
                continue;
            };
            let screen = super::previews::painting(screen.clone());
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

impl beui::native::Draw for AppDrawing {
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
                        "The active renderer has no app surface presenter.".to_owned(),
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
            let mut pieces = crate::layers::Pieces::default();
            for blit in blits.iter() {
                let (frames, region, surfaces) = {
                    let mut shared = blit.shared.borrow_mut();
                    let frames = shared.take_frames();
                    let region = match blit.layer {
                        Some(_) => None,
                        None => shared.placement(blit.screen).copied(),
                    };
                    let surfaces = shared.surfaces();
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
                // Every app surface is drawn from its layer (crate::layers), as the web places
                // its canvases from the frame's layers.
                let layer = match blit.layer {
                    Some(anchored) => {
                        platform
                            .size(blit.surface, anchored.surface)
                            .and_then(|size| {
                                let piece =
                                    pieces.next(blit.surface, u64::from(anchored.surface), true);
                                blit.anchored_layer(anchored, size, piece)
                            })
                    }
                    None => region.and_then(|placement| {
                        let texture = platform.size(blit.surface, placement.surface);
                        let piece = pieces.next(blit.surface, blit.screen.0, false);
                        blit.screen_layer(&placement, texture, piece)
                    }),
                };
                blit.status.set(match failure {
                    Some(error) => PresenterState::Failed(error),
                    None => PresenterState::Presenting,
                });
                if let Some(layer) = layer {
                    let surface = match layer.surface {
                        Surface::Screen { surface, .. } | Surface::Anchored { surface, .. } => {
                            surface
                        }
                        Surface::Root(_) | Surface::Host(_) => {
                            unreachable!("a blit's layer is an app's surface")
                        }
                    };
                    placed.push((
                        blit.surface,
                        surface,
                        platform.shown(crate::layers::texture_quad(&layer, &at)),
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
    let Some(gpu) = setup.get::<beui_renderer_wgpu::present::GpuSetup>() else {
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

pub(super) fn keep(slot: u32, surfaces: &[u32]) {
    PRESENTER.with(|presenter| {
        if let Some(platform) = presenter
            .borrow_mut()
            .as_mut()
            .and_then(|presenter| presenter.platform.as_mut())
        {
            platform.retain(slot, surfaces);
        }
    });
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

const UNSUPPORTED: &str = "This build has no presenter for that app surface.";

/// Which of a frame's blits, each its target and whether it is an anchored view's, the web
/// puts above the host's canvas: a screen over an anchored view painted before it (a child
/// placed in one), which must show above that view as it does natively. Every other screen
/// stays below, under the host's own chrome. Anchored views place themselves (overlay.rs).
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) fn lifted(blits: impl IntoIterator<Item = (Rect, bool)>) -> Vec<bool> {
    let mut anchored: Vec<Rect> = Vec::new();
    blits
        .into_iter()
        .map(|(target, layer)| {
            if layer {
                anchored.push(target);
                return false;
            }
            anchored.iter().any(|view| view.intersects(target))
        })
        .collect()
}

#[cfg(test)]
mod tests;
