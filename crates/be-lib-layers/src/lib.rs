//! The layer list (plans/host_as_platform_adapter.md, The layer list): for each OS window, its
//! surfaces in z-order, each with a 2D affine transform, a clip, an opacity, whether it is shown
//! and where it takes input. The host composites it (natively in its wgpu pass, on the web as
//! CSS on the canvases it places), routes input over it with the forwarding router
//! (be-lib-input-router, through `HitTest` here) and finds a window's damage from it.
//!
//! It names surfaces only by handle and holds no pixels, so it means the same to the host, the
//! root guest that will produce it and Space, and to any framework.

use be_lib_geometry::{Pos2, Rect, Vec2, pos2, vec2};
use be_lib_input_router::HitTest;
use be_lib_stable_hash::HashMap;
use be_lib_ui_types::WindowId;
use serde::{Deserialize, Serialize};

/// A layer's identity, stable across frames while it shows the same thing, so damage can tell
/// a layer that moved from one that came.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct LayerId(pub u64);

impl LayerId {
    /// The layer showing the root's surface `Surface::Root(index)`: hashed from its index under
    /// a tag of its own, as the host hashes a drawing's pieces, so it is no other layer's id.
    pub fn root(index: u32) -> Self {
        Self(std::hash::BuildHasher::hash_one(
            &be_lib_stable_hash::State::default(),
            ("root", index),
        ))
    }
}

/// What a layer shows, by handle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Surface {
    /// One of the root's own surfaces: the host's UI today, the root guest's from step 4. It
    /// may have several, such as one below its children and one above.
    Root(u32),
    /// A screen of an instance, `surface` being the app's surface. `instance` is be-host's
    /// runtime's slot, or for an instance a root guest spawned, its `instance` resource's op
    /// (`be3:sandbox`), which both the root and its host know it by.
    Screen { instance: u64, surface: u32 },
    /// An anchored view's part of an instance's anchored surface (popups, menus).
    Anchored { instance: u64, surface: u32 },
    /// A surface the host owns, such as a program window.
    Host(u64),
}

impl Surface {
    /// The instance whose surface this is.
    pub fn instance(self) -> Option<u64> {
        match self {
            Self::Screen { instance, .. } | Self::Anchored { instance, .. } => Some(instance),
            Self::Root(_) | Self::Host(_) => None,
        }
    }
}

/// A 2D affine transform, from a surface's pixels to its window's points:
/// `x' = xx·x + yx·y + x`, `y' = xy·x + yy·y + y`, CSS's `matrix(xx, xy, yx, yy, x, y)`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Affine {
    pub xx: f32,
    pub xy: f32,
    pub yx: f32,
    pub yy: f32,
    pub x: f32,
    pub y: f32,
}

impl Affine {
    pub const IDENTITY: Self = Self {
        xx: 1.0,
        xy: 0.0,
        yx: 0.0,
        yy: 1.0,
        x: 0.0,
        y: 0.0,
    };

    /// The transform taking a surface's pixel `(1, 0)` step to `across`, its `(0, 1)` step to
    /// `down`, and its origin to `origin`.
    pub fn from_axes(across: Vec2, down: Vec2, origin: Pos2) -> Self {
        Self {
            xx: across.x,
            xy: across.y,
            yx: down.x,
            yy: down.y,
            x: origin.x,
            y: origin.y,
        }
    }

    pub fn scaling(scale: f32) -> Self {
        Self {
            xx: scale,
            yy: scale,
            ..Self::IDENTITY
        }
    }

    pub fn apply(&self, point: Pos2) -> Pos2 {
        pos2(
            self.xx * point.x + self.yx * point.y + self.x,
            self.xy * point.x + self.yy * point.y + self.y,
        )
    }

    pub fn across(&self) -> Vec2 {
        vec2(self.xx, self.xy)
    }

    pub fn down(&self) -> Vec2 {
        vec2(self.yx, self.yy)
    }

    pub fn origin(&self) -> Pos2 {
        pos2(self.x, self.y)
    }

    /// This transform followed by `then`.
    pub fn then(&self, then: &Self) -> Self {
        let origin = then.apply(self.origin());
        let across = then.apply(pos2(self.xx, self.xy)) - then.origin();
        let down = then.apply(pos2(self.yx, self.yy)) - then.origin();
        Self::from_axes(across, down, origin)
    }

    pub fn inverse(&self) -> Option<Self> {
        let determinant = self.xx * self.yy - self.yx * self.xy;
        if !determinant.is_finite() || determinant.abs() <= f32::EPSILON * f32::EPSILON {
            return None;
        }
        let (xx, xy, yx, yy) = (
            self.yy / determinant,
            -self.xy / determinant,
            -self.yx / determinant,
            self.xx / determinant,
        );
        Some(Self {
            xx,
            xy,
            yx,
            yy,
            x: -(xx * self.x + yx * self.y),
            y: -(xy * self.x + yy * self.y),
        })
    }

    /// The four corners of `rect` once transformed, from its top left clockwise.
    pub fn corners(&self, rect: Rect) -> [Pos2; 4] {
        [
            rect.min,
            pos2(rect.max.x, rect.min.y),
            rect.max,
            pos2(rect.min.x, rect.max.y),
        ]
        .map(|corner| self.apply(corner))
    }

    /// The upright bounds of `rect` once transformed.
    pub fn bounds(&self, rect: Rect) -> Rect {
        let corners = self.corners(rect);
        corners.iter().skip(1).fold(
            Rect::from_min_max(corners[0], corners[0]),
            |bounds, corner| bounds.union(Rect::from_min_max(*corner, *corner)),
        )
    }
}

/// Where a layer takes input, in its surface's pixels.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum Area {
    /// All of what the layer shows.
    #[default]
    Whole,
    /// These rects of it.
    Region(Vec<Rect>),
    /// None of it.
    Nowhere,
}

impl Area {
    fn contains(&self, point: Pos2) -> bool {
        match self {
            Self::Whole => true,
            Self::Region(rects) => rects.iter().any(|rect| rect.contains(point)),
            Self::Nowhere => false,
        }
    }
}

/// How a layer takes input (plans/host_as_platform_adapter.md, Input).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LayerInput {
    /// Where a pointer lands on it.
    pub hit: Area,
    /// Where its placer's camera takes wheel, pinch and pan-drag first.
    pub gestures: Area,
    /// Whether it captures presses, as a scrim or a popup does: a press anywhere outside it goes
    /// to its placer, to dismiss it, and to nothing under it.
    pub captures: bool,
    /// Whether a press gives it the keyboard (popups and the on-screen keyboard never take it).
    pub focuses: bool,
    /// The layer whose surface placed it, so a gesture reaches the right camera.
    pub placer: Option<LayerId>,
}

impl Default for LayerInput {
    fn default() -> Self {
        Self {
            hit: Area::Whole,
            gestures: Area::Nowhere,
            captures: false,
            focuses: true,
            placer: None,
        }
    }
}

/// One surface placed in a window.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Layer {
    pub id: LayerId,
    pub surface: Surface,
    /// The size of what the surface holds, in its pixels.
    pub size: (u32, u32),
    /// From the surface's pixels to the window's points.
    pub transform: Affine,
    /// The part of the surface shown, in its pixels.
    pub clip: Rect,
    /// The rounding of the surface's own corners, in the window's points: it rounds the corners
    /// of the clip that meet the surface's edges.
    pub corner_radius: f32,
    pub opacity: f32,
    pub shown: bool,
    pub input: LayerInput,
}

impl Layer {
    /// The window's points this layer may draw over.
    pub fn bounds(&self) -> Rect {
        self.transform.bounds(self.clip)
    }

    /// A point of the window in the surface's pixels.
    pub fn local(&self, at: Pos2) -> Option<Pos2> {
        Some(self.transform.inverse()?.apply(at))
    }

    /// A rect of the surface (an input method's field, damage) in the window's points.
    pub fn to_window(&self, rect: Rect) -> Rect {
        self.transform.bounds(rect)
    }

    /// Whether a pointer at `at` lands on this layer.
    pub fn takes(&self, at: Pos2) -> bool {
        self.shown
            && self
                .local(at)
                .is_some_and(|local| self.clip.contains(local) && self.input.hit.contains(local))
    }

    /// Whether the clip reaches each of the surface's corners, from its top left clockwise: the
    /// corners `corner_radius` rounds.
    pub fn rounded(&self) -> [bool; 4] {
        let (width, height) = (self.size.0 as f32, self.size.1 as f32);
        let left = self.clip.min.x <= 0.0;
        let top = self.clip.min.y <= 0.0;
        let right = self.clip.max.x >= width;
        let bottom = self.clip.max.y >= height;
        [left && top, right && top, right && bottom, left && bottom]
    }

    /// Whether this layer and `other` place their surfaces alike: same surface, transform,
    /// clip, rounding, opacity and visibility.
    fn placed_as(&self, other: &Self) -> bool {
        self.surface == other.surface
            && self.size == other.size
            && self.transform == other.transform
            && self.clip == other.clip
            && self.corner_radius == other.corner_radius
            && self.opacity == other.opacity
            && self.shown == other.shown
    }
}

/// One window's layers, bottom first.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct WindowLayers {
    pub window: Option<WindowId>,
    pub layers: Vec<Layer>,
}

impl WindowLayers {
    pub fn layer(&self, id: LayerId) -> Option<&Layer> {
        self.layers.iter().find(|layer| layer.id == id)
    }

    /// What changed on the window between `self` and `next`, in its points, for a display that
    /// redraws only part of itself (e-paper's `FB_DAMAGE_CLIPS`): each shown surface's own
    /// damage (`damage`, in its pixels) through its layer, the old and new bounds of every layer
    /// that moved, appeared, went or changed its opacity, clip or visibility, and where two layers
    /// that both lists place alike overlap and swapped order. `damage` answers `None` for a
    /// surface redrawn whole.
    pub fn damage(
        &self,
        next: &WindowLayers,
        damage: impl Fn(&Layer) -> Option<Vec<Rect>>,
    ) -> Vec<Rect> {
        let before: HashMap<LayerId, (usize, &Layer)> = self
            .layers
            .iter()
            .enumerate()
            .map(|(rank, layer)| (layer.id, (rank, layer)))
            .collect();
        let after: HashMap<LayerId, &Layer> =
            next.layers.iter().map(|layer| (layer.id, layer)).collect();
        let mut out = Vec::new();
        let whole = |out: &mut Vec<Rect>, layer: &Layer| {
            if layer.shown && layer.bounds().is_positive() {
                out.push(layer.bounds());
            }
        };
        // The shown layers placed alike in both lists, in their new order, with their old rank.
        let mut kept: Vec<(usize, &Layer)> = Vec::new();
        for layer in &next.layers {
            match before.get(&layer.id) {
                None => whole(&mut out, layer),
                Some((_, old)) if !old.placed_as(layer) => {
                    whole(&mut out, old);
                    whole(&mut out, layer);
                }
                Some((rank, _)) => {
                    if layer.shown {
                        kept.push((*rank, layer));
                    }
                }
            }
        }
        for layer in &self.layers {
            if !after.contains_key(&layer.id) {
                whole(&mut out, layer);
            }
        }
        // A pair that swapped order shows differently only where both show. A layer that moved
        // or changed is damaged whole already, which covers any pair it is in.
        for (index, (rank, lower)) in kept.iter().enumerate() {
            for (other, upper) in &kept[index + 1..] {
                if other < rank {
                    let overlap = lower.bounds().intersect(upper.bounds());
                    if overlap.is_positive() {
                        out.push(overlap);
                    }
                }
            }
        }
        for (_, layer) in kept {
            match damage(layer) {
                None => whole(&mut out, layer),
                Some(rects) => out.extend(rects.into_iter().filter_map(|rect| {
                    let shown = rect.intersect(layer.clip);
                    shown.is_positive().then(|| layer.to_window(shown))
                })),
            }
        }
        out
    }
}

/// The 2D hit test the forwarding router routes a window's input with: against the last
/// committed layers, so a surface that moved this frame is hit where it was, as in a browser.
impl HitTest for WindowLayers {
    type Target = LayerId;

    fn hit(&self, at: Pos2) -> Option<LayerId> {
        for layer in self.layers.iter().rev() {
            if !layer.shown {
                continue;
            }
            if layer.takes(at) {
                return Some(layer.id);
            }
            if layer.input.captures {
                return layer.input.placer;
            }
        }
        None
    }

    fn local(&self, target: LayerId, at: Pos2) -> Option<Pos2> {
        self.layer(target)?.local(at)
    }

    fn placer(&self, target: LayerId) -> Option<LayerId> {
        self.layer(target)?.input.placer
    }

    fn grants_gestures(&self, target: LayerId, at: Pos2) -> bool {
        self.layer(target).is_some_and(|layer| {
            layer
                .local(at)
                .is_some_and(|local| layer.input.gestures.contains(local))
        })
    }

    fn takes_focus(&self, target: LayerId) -> bool {
        self.layer(target).is_some_and(|layer| layer.input.focuses)
    }

    /// The layer's surface's pixels a window point covers: a zoomed screen's or program
    /// window's are fewer.
    fn scale(&self, target: LayerId) -> f32 {
        self.layer(target).map_or(1.0, |layer| {
            1.0 / layer.transform.across().length().max(f32::EPSILON)
        })
    }
}

/// The layers of every window the root shows, the root's answer each frame.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LayerList {
    pub windows: Vec<WindowLayers>,
}

impl LayerList {
    /// The list as the root guest's frame carries it (be-wasm-root-api's `frame-done`).
    pub fn encode(&self) -> Vec<u8> {
        postcard::to_allocvec(self).expect("a layer list always encodes")
    }

    /// A list `encode` made, or why the bytes are not one.
    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        postcard::from_bytes(bytes).map_err(|error| format!("not a layer list: {error}"))
    }
}

#[cfg(test)]
mod tests;
