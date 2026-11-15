use std::any::Any;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use beui_reactive::{Stored, on_cleanup, store};

use crate::document::Document;
use crate::node::{Element, NodeOf, Rects};
use crate::painter::Painter;
use beui_types::Direction;
use beui_types::NodeId;
use beui_types::{Rect, Vec2};

/// Where an embed was laid out, in window coordinates, and the clip around it. `raster` is
/// its window's raster over the document's scale (1 unless the host draws the window
/// finer or coarser), announced again when a `WindowReport::Rastered` changes it, so the
/// host can draw the embed as sharp as the window.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EmbedPlacement {
    pub rect: Rect,
    pub clip: Rect,
    pub radius: f32,
    pub raster: f32,
}

type PlaceListener = Box<dyn Fn(Option<EmbedPlacement>)>;
type InertListener = Box<dyn Fn(bool)>;

#[derive(Default)]
pub struct EmbedState {
    pub node: Cell<Option<NodeId>>,
    placement: Cell<Option<EmbedPlacement>>,
    concealed: Cell<bool>,
    listener: RefCell<Option<PlaceListener>>,
    inert: Cell<bool>,
    inert_listener: RefCell<Option<InertListener>>,
}

impl EmbedState {
    fn shown(&self) -> Option<EmbedPlacement> {
        self.placement.get().filter(|_| !self.concealed.get())
    }

    fn place(&self, placement: Option<EmbedPlacement>) {
        let was = self.shown();
        self.placement.set(placement);
        self.announce(was);
    }

    pub(crate) fn rerasterise(&self, raster: f32) {
        if let Some(placement) = self.placement.get()
            && placement.raster != raster
        {
            self.place(Some(EmbedPlacement {
                raster,
                ..placement
            }));
        }
    }

    fn make_inert(&self, inert: bool) {
        if self.inert.replace(inert) == inert {
            return;
        }
        if let Some(listener) = self.inert_listener.borrow().as_ref() {
            listener(inert);
        }
    }

    pub(crate) fn conceal(&self, concealed: bool) {
        let was = self.shown();
        self.concealed.set(concealed);
        self.announce(was);
    }

    fn announce(&self, was: Option<EmbedPlacement>) {
        let shown = self.shown();
        if shown == was {
            return;
        }
        if let Some(listener) = self.listener.borrow().as_ref() {
            listener(shown);
        }
    }
}

/// How an `Embed` hands its placement to whoever draws what it reserves; owned by the scope
/// that made it. `on_inert` hears when a layer modal to assistive technology covers it, so
/// a host leaves that app out of its description.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct EmbedSlot(pub Stored<EmbedState>);

impl Default for EmbedSlot {
    fn default() -> Self {
        Self::new()
    }
}

impl EmbedSlot {
    pub fn new() -> Self {
        Self(store(EmbedState::default()))
    }

    pub fn node(&self) -> Option<NodeId> {
        self.0.with(|state| state.node.get())
    }

    pub fn placement(&self) -> Option<EmbedPlacement> {
        self.0.with(EmbedState::shown)
    }

    pub fn on_place(&self, listener: impl Fn(Option<EmbedPlacement>) + 'static) {
        let state = self.0.value();
        *state.listener.borrow_mut() = Some(Box::new(listener));
        on_cleanup(move || drop(state.listener.take()));
    }

    pub fn inert(&self) -> bool {
        self.0.with(|state| state.inert.get())
    }

    pub fn on_inert(&self, listener: impl Fn(bool) + 'static) {
        let state = self.0.value();
        *state.inert_listener.borrow_mut() = Some(Box::new(listener));
        on_cleanup(move || drop(state.inert_listener.take()));
    }
}

/// Reserves a rectangle for something outside the document (an editor the host composites
/// behind the surface) and, unless `punch` is false, cuts it out of the surface so what is
/// behind shows through. What the host draws there covers the document, overlays aside,
/// and nodes marked `above_embeds` that paint after it.
pub struct EmbedNode {
    child: Option<NodeId>,
    punch: bool,
    rotation: f32,
    radius: f32,
    state: Rc<EmbedState>,
    own: Option<NodeId>,
}

impl EmbedNode {
    fn forget(&self) {
        if self.own.is_some() && self.state.node.get() == self.own {
            self.state.place(None);
        }
    }
}

impl Element for EmbedNode {
    fn paints_from_layout(&self) -> bool {
        false
    }

    fn measure(&self, doc: &mut Document, painter: &Painter, available: Vec2) -> Vec2 {
        match self.child {
            Some(child) => crate::layout::measure(doc, painter, child, available),
            None => Vec2::ZERO,
        }
    }

    fn min_width(&self, doc: &mut Document, painter: &Painter) -> f32 {
        self.child
            .map_or(0.0, |child| crate::layout::min_width(doc, painter, child))
    }

    fn least_length(
        &self,
        doc: &mut Document,
        painter: &Painter,
        direction: Direction,
        cross: f32,
    ) -> Option<f32> {
        Some(self.child.map_or(0.0, |child| {
            crate::layout::least_length(doc, painter, child, direction, cross)
        }))
    }

    fn layout(&mut self, doc: &mut Document, painter: &Painter, rect: Rect, out: &Rects) {
        let grid = doc.pixel_grid();
        let placed = grid.snap_rect(painter.to_screen(rect));
        self.state.place(Some(EmbedPlacement {
            rect: placed,
            clip: grid
                .snap_rect(painter.to_screen(painter.clip_rect()))
                .intersect(placed),
            radius: painter
                .to_screen(Rect::from_min_size(rect.min, Vec2::new(self.radius, 0.0)))
                .width(),
            raster: self.own.map_or(1.0, |own| doc.raster_of(own)),
        }));
        if let Some(child) = self.child {
            crate::layout::layout(doc, painter, child, rect, out);
        }
    }

    fn paint(&self, doc: &Document, painter: &Painter, rects: &Rects, rect: Rect) {
        if self.punch {
            painter
                .rotated(rect.center(), self.rotation)
                .punch(rect, self.radius);
        }
        if let Some(child) = self.child {
            crate::paint::paint(doc, painter, rects, child);
        }
    }

    fn paints(&self) -> bool {
        self.punch
    }

    fn unplaced(&mut self, _doc: &mut Document) {
        self.forget();
    }

    fn detached(&mut self) {
        self.forget();
    }

    fn children(&self) -> Vec<NodeId> {
        self.child.into_iter().collect()
    }

    fn remove_child(&mut self, child: NodeId) {
        if self.child == Some(child) {
            self.child = None;
        }
    }

    fn kind(&self) -> &'static str {
        "embed"
    }

    fn detail(&self) -> Option<String> {
        let placement = self.state.shown()?;
        Some(format!(
            "{:.0}x{:.0}",
            placement.rect.width(),
            placement.rect.height()
        ))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Document {
    pub fn raster_of(&self, node: NodeId) -> f32 {
        self.context().map_or(1.0, |ctx| {
            ctx.window_pixels_per_point(self.window_of(node)) / ctx.pixels_per_point()
        })
    }

    pub(crate) fn rerasterise_embeds(&mut self) {
        let embeds: Vec<Rc<EmbedState>> = self
            .embeds
            .iter()
            .filter_map(std::rc::Weak::upgrade)
            .collect();
        for embed in embeds {
            if let Some(node) = embed.node.get().filter(|node| self.contains(*node)) {
                embed.rerasterise(self.raster_of(node));
            }
        }
    }

    pub(crate) fn refresh_inert_embeds(&mut self) {
        let embeds: Vec<Rc<EmbedState>> = self
            .embeds
            .iter()
            .filter_map(std::rc::Weak::upgrade)
            .collect();
        for embed in embeds {
            if let Some(node) = embed.node.get().filter(|node| self.contains(*node)) {
                embed.make_inert(self.inert(node));
            }
        }
    }

    pub fn create_embed(&mut self, state: Rc<EmbedState>) -> NodeOf<EmbedNode> {
        state.concealed.set(self.hidden);
        if !self
            .embeds
            .iter()
            .any(|held| std::ptr::eq(held.as_ptr(), Rc::as_ptr(&state)))
        {
            self.embeds.push(Rc::downgrade(&state));
        }
        let embed = self.arena.insert(EmbedNode {
            child: None,
            punch: false,
            rotation: 0.0,
            radius: 0.0,
            state,
            own: None,
        });
        self.arena.touch_mut_as::<EmbedNode>(embed).own = Some(embed.id());
        embed
    }

    pub fn set_embed_child(&mut self, embed: NodeOf<EmbedNode>, child: NodeId) {
        if self.arena.get_as::<EmbedNode>(embed).child != Some(child) {
            self.arena.get_mut_as::<EmbedNode>(embed).child = Some(child);
        }
    }

    pub fn set_embed_punch(&mut self, embed: NodeOf<EmbedNode>, punch: bool) {
        if self.arena.get_as::<EmbedNode>(embed).punch != punch {
            self.arena.paint_mut_as::<EmbedNode>(embed).punch = punch;
        }
    }

    pub fn set_embed_rotation(&mut self, embed: NodeOf<EmbedNode>, rotation: f32) {
        if self.arena.get_as::<EmbedNode>(embed).rotation != rotation {
            self.arena.paint_mut_as::<EmbedNode>(embed).rotation = rotation;
        }
    }

    pub fn set_embed_radius(&mut self, embed: NodeOf<EmbedNode>, radius: f32) {
        if self.arena.get_as::<EmbedNode>(embed).radius != radius {
            self.arena.get_mut_as::<EmbedNode>(embed).radius = radius;
        }
    }
}
