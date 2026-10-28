use std::any::Any;
use std::rc::Rc;

use crate::color::Color;
use crate::geometry::{Pos2, Rect, Vec2, pos2, vec2};
use crate::painter::{Corners, Painter};

use crate::base::frame::{FrameNode, FrameStyle};
use crate::callback::{Callback, NodeRef};
use crate::document::Document;
use crate::layer::{Dismiss, LayerLevel, LayerPolicy};
use crate::node::{Element, NodeId, NodeOf, Rects};
use crate::window::WindowId;

pub use beui_types::layer::OverlayAnchor;

#[derive(Clone, Copy, PartialEq, Debug)]
struct Pin {
    from_end: [bool; 2],
    offset: Vec2,
    size: Vec2,
}

impl Pin {
    fn new(rect: Rect, within: Rect) -> Self {
        let from_end = [
            rect.center().x > within.center().x,
            rect.center().y > within.center().y,
        ];
        let edge = pos2(
            if from_end[0] {
                within.max.x
            } else {
                within.min.x
            },
            if from_end[1] {
                within.max.y
            } else {
                within.min.y
            },
        );
        Self {
            from_end,
            offset: rect.min - edge,
            size: rect.size(),
        }
    }

    fn place(self, within: Rect) -> Rect {
        let edge = pos2(
            if self.from_end[0] {
                within.max.x
            } else {
                within.min.x
            },
            if self.from_end[1] {
                within.max.y
            } else {
                within.min.y
            },
        );
        Rect::from_min_size(edge + self.offset, self.size)
    }
}

pub use beui_types::layer::Placement;

pub use beui_types::window::WindowSpec;

const TIP_RADIUS: f32 = 2.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum BeakSide {
    Top,
    Bottom,
}

#[derive(Clone, Copy, PartialEq, Debug)]
struct Pointing {
    side: BeakSide,
    at: f32,
}

pub struct BeakNode {
    reach: f32,
    style: FrameStyle,
    pointing: Option<Pointing>,
}

fn beak_room(reach: f32, style: &FrameStyle, rect: Rect) -> (f32, f32) {
    let radius = style.radius.largest();
    let reach = reach + outline_width(style);
    (rect.left() + radius + reach, rect.right() - radius - reach)
}

fn outline_width(style: &FrameStyle) -> f32 {
    style.outline.map_or(0.0, |outline| outline.width)
}

impl Element for BeakNode {
    fn measure(&self, _doc: &mut Document, _painter: &Painter, _available: Vec2) -> Vec2 {
        Vec2::ZERO
    }

    fn layout(&mut self, _doc: &mut Document, _painter: &Painter, _rect: Rect, _out: &Rects) {}

    fn paint(&self, doc: &Document, painter: &Painter, _rects: &Rects, rect: Rect) {
        let Some(pointing) = self.pointing.filter(|_| self.reach > 0.0) else {
            return;
        };
        let (lowest, highest) = beak_room(self.reach, &self.style, rect);
        if lowest > highest {
            return;
        }
        let x = (rect.left() + pointing.at).clamp(lowest, highest);
        let width = outline_width(&self.style);
        let half = (self.reach + width) / std::f32::consts::SQRT_2;
        let around = rect.expand(self.reach + 1.0);
        let (y, clip, tip) = match pointing.side {
            BeakSide::Top => (
                rect.top() + width,
                Rect::from_min_max(around.min, pos2(around.right(), rect.top() + width)),
                Corners::new(TIP_RADIUS, 0.0, 0.0, 0.0),
            ),
            BeakSide::Bottom => (
                rect.bottom() - width,
                Rect::from_min_max(pos2(around.left(), rect.bottom() - width), around.max),
                Corners::new(0.0, 0.0, TIP_RADIUS, 0.0),
            ),
        };
        let centre = pos2(x, y);
        let square = Rect::from_center_size(centre, Vec2::splat(2.0 * half));
        let turned = painter
            .with_clip_rect(clip)
            .rotated(centre, std::f32::consts::FRAC_PI_4);
        turned.rect_filled(square, tip, doc.resolve_color(self.style.fill));
        if let Some(outline) = self.style.outline {
            turned.rect_stroke(square, tip, outline.width, doc.resolve_color(outline.color));
        }
    }

    fn children(&self) -> Vec<NodeId> {
        Vec::new()
    }

    fn remove_child(&mut self, _child: NodeId) {}

    fn kind(&self) -> &'static str {
        "beak"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

pub struct OverlayNode {
    pub(crate) content: Option<NodeId>,
    pub(crate) window: Option<WindowSpec>,
    pub(crate) shown_in: Option<WindowId>,
    scrim: NodeOf<FrameNode>,
    dims: bool,
    scrim_color: Color,
    pub(crate) open: bool,
    pub(crate) anchor: OverlayAnchor,
    anchored: Option<Rect>,
    pinned: Option<Pin>,
    pointer: Option<NodeRef>,
    beak: Option<NodeOf<BeakNode>>,
    placement: Placement,
    pub(crate) policy: LayerPolicy,
    pub(crate) level: LayerLevel,
    pub(crate) traps_focus: bool,
    pub(crate) trigger: Option<NodeRef>,
    pub(crate) on_dismiss: Callback<Dismiss>,
}

impl OverlayNode {
    fn new(scrim: NodeOf<FrameNode>, anchor: OverlayAnchor, placement: Placement) -> Self {
        Self {
            content: None,
            window: None,
            shown_in: None,
            scrim,
            dims: false,
            scrim_color: Color::TRANSPARENT,
            open: false,
            anchor,
            anchored: None,
            pinned: None,
            pointer: None,
            beak: None,
            placement,
            policy: LayerPolicy::BlocksAll,
            level: LayerLevel::Normal,
            traps_focus: true,
            trigger: None,
            on_dismiss: Callback::empty(),
        }
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn policy(&self) -> LayerPolicy {
        self.policy
    }

    pub(crate) fn anchor_node(&self) -> Option<NodeId> {
        match &self.anchor {
            OverlayAnchor::Node(node) | OverlayAnchor::Pinned { node, .. } => node.try_get(),
            OverlayAnchor::Point(_) | OverlayAnchor::Rect(_) => None,
        }
    }

    fn pinned_rect(&mut self, rect: Rect, within: Option<Rect>) -> Rect {
        let Some(within) = within else {
            return rect;
        };
        let pin = *self.pinned.get_or_insert_with(|| Pin::new(rect, within));
        pin.place(within)
    }
}

impl OverlayNode {
    fn place_content(
        &mut self,
        doc: &mut Document,
        painter: &Painter,
        content: NodeId,
        out: &Rects,
    ) {
        let viewport = doc.viewport_rect();
        let anchored = self.anchor_node().and_then(|id| out.get(&id));
        self.anchored = anchored;
        let target = match self.anchor.clone() {
            OverlayAnchor::Pinned { rect, .. } => Some(self.pinned_rect(rect, anchored)),
            OverlayAnchor::Node(_) | OverlayAnchor::Point(_) | OverlayAnchor::Rect(_) => anchored,
        };
        let area = viewport;
        let anchor_rect = match &self.anchor {
            OverlayAnchor::Node(_) => anchored.unwrap_or(area),
            OverlayAnchor::Pinned { .. } => target.unwrap_or(area),
            OverlayAnchor::Point(pos) => Rect::from_min_size(*pos, Vec2::ZERO),
            OverlayAnchor::Rect(rect) => *rect,
        };
        let inside = matches!(
            self.placement,
            Placement::InsideTop
                | Placement::InsideTopEnd
                | Placement::InsideBottom
                | Placement::InsideBottomEnd
        );
        let available = match (inside, self.placement) {
            (true, _) => anchor_rect.intersect(area).size().max(Vec2::ZERO),
            (false, Placement::HangFrom(gap)) => {
                let room = area.shrink(f32::from(gap));
                let top = (anchor_rect.top() + f32::from(gap)).clamp(room.top(), room.bottom());
                Vec2::new(room.width(), room.bottom() - top).max(Vec2::ZERO)
            }
            (false, Placement::Popover { margin, .. }) => {
                area.shrink(f32::from(margin)).size().max(Vec2::ZERO)
            }
            (false, _) => area.size(),
        };
        let measured = crate::layout::measure(doc, painter, content, available);
        let content_size = match inside {
            true => measured.min(available),
            false => measured,
        };
        let rect = beui_types::layer::resolve_rect(area, anchor_rect, self.placement, content_size);
        crate::layout::layout(doc, painter, content, rect, out);
        self.stand_off(doc, painter, content, rect, anchor_rect, out);
        self.point_at(doc, painter, anchor_rect, out);
        if self.policy == LayerPolicy::BlocksParent
            && self.dims
            && let Some(blocked) = anchored
        {
            crate::layout::layout(doc, painter, self.scrim.id(), blocked, out);
        }
    }

    fn pointer_frame(&self, doc: &Document) -> Option<NodeOf<FrameNode>> {
        self.pointer
            .as_ref()
            .and_then(NodeRef::try_get)
            .and_then(|id| doc.arena.kind_of::<FrameNode>(id))
    }

    fn stand_off(
        &self,
        doc: &mut Document,
        painter: &Painter,
        content: NodeId,
        rect: Rect,
        anchor: Rect,
        out: &Rects,
    ) {
        if !matches!(self.placement, Placement::Popover { .. }) {
            return;
        }
        let Some(frame) = self
            .pointer_frame(doc)
            .and_then(|frame| out.get(&frame.id()))
        else {
            return;
        };
        let shift = match (rect.top() >= anchor.bottom(), rect.bottom() <= anchor.top()) {
            (true, _) => rect.top() - frame.top(),
            (false, true) => rect.bottom() - frame.bottom(),
            (false, false) => 0.0,
        };
        if shift != 0.0 {
            crate::layout::layout(doc, painter, content, rect.translate(vec2(0.0, shift)), out);
        }
    }

    fn point_at(&self, doc: &mut Document, painter: &Painter, anchor: Rect, out: &Rects) {
        let Some(beak) = self.beak else {
            return;
        };
        let frame = self.pointer_frame(doc);
        let placed = frame.and_then(|frame| out.get(&frame.id()));
        let style = frame.map_or_else(FrameStyle::default, |frame| {
            doc.arena.get_as::<FrameNode>(frame).style
        });
        let reach = doc.arena.get_as::<BeakNode>(beak).reach;
        let pointing = placed.and_then(|rect| {
            if !matches!(self.placement, Placement::Popover { .. }) {
                return None;
            }
            let (lowest, highest) = beak_room(reach, &style, rect);
            let centre = anchor.center().x;
            if centre < lowest || centre > highest {
                return None;
            }
            let side = match (rect.top() >= anchor.bottom(), rect.bottom() <= anchor.top()) {
                (true, _) => BeakSide::Top,
                (false, true) => BeakSide::Bottom,
                (false, false) => return None,
            };
            Some(Pointing {
                side,
                at: centre - rect.left(),
            })
        });
        let held = doc.arena.get_as::<BeakNode>(beak);
        if held.pointing != pointing || held.style != style {
            let node = doc.arena.paint_mut_as::<BeakNode>(beak);
            node.pointing = pointing;
            node.style = style;
        }
        if let Some(rect) = placed {
            crate::layout::layout(doc, painter, beak.id(), rect, out);
        }
    }

    pub(crate) fn dims(&self) -> bool {
        self.dims
    }

    pub(crate) fn scrim_color(&self) -> Color {
        self.scrim_color
    }

    pub(crate) fn scrim_covers(&self) -> bool {
        match self.policy {
            LayerPolicy::BlocksAll => true,
            LayerPolicy::BlocksParent => self.dims,
            LayerPolicy::Pass | LayerPolicy::Inert => false,
        }
    }

    fn anchor_moved(&self, out: &Rects) -> bool {
        self.anchor_node()
            .and_then(|id| out.get(&id))
            .is_some_and(|rect| self.anchored != Some(rect))
    }
}

impl Element for OverlayNode {
    fn paints_from_layout(&self) -> bool {
        false
    }

    fn measure(&self, _doc: &mut Document, _painter: &Painter, _available: Vec2) -> Vec2 {
        Vec2::ZERO
    }

    fn layout(&mut self, doc: &mut Document, painter: &Painter, _rect: Rect, out: &Rects) {
        if !self.open {
            return;
        }
        let Some(content) = self.content else {
            return;
        };
        let real = self
            .window
            .as_ref()
            .filter(|window| doc.hosts_window(window));
        let shown_in = self.shown_in.filter(|window| doc.shows_in(*window));
        let painter = &painter.ctx().window_painter(match (real, shown_in) {
            (Some(window), _) => window.id,
            (None, Some(window)) => window,
            (None, None) => doc.viewport_window(),
        });
        if let Some(window) = real {
            let size = painter.ctx().window_size(window.id).unwrap_or(window.size);
            let rect = painter
                .ctx()
                .keyboard_trimmed(window.id, Rect::from_min_size(Pos2::ZERO, size));
            let outer = doc.swap_window_viewport(Some((window.id, rect)));
            crate::layout::layout(doc, painter, content, rect, out);
            doc.swap_window_viewport(outer);
            return;
        }
        let outer = shown_in.map(|window| {
            let viewport = doc.window_viewport(window);
            doc.swap_window_viewport(viewport)
        });
        let viewport = doc.viewport_rect();
        if self.policy == LayerPolicy::BlocksAll {
            crate::layout::layout(doc, painter, self.scrim.id(), viewport, out);
        }
        self.place_content(doc, painter, content, out);
        if let Some(outer) = outer {
            doc.swap_window_viewport(outer);
        }
    }

    fn paint(&self, _doc: &Document, _painter: &Painter, _rects: &Rects, _rect: Rect) {}

    fn paints(&self) -> bool {
        false
    }

    fn children(&self) -> Vec<NodeId> {
        let mut children = vec![self.scrim.id()];
        children.extend(self.content);
        children.extend(self.beak.map(NodeOf::id));
        children
    }

    fn remove_child(&mut self, child: NodeId) {
        if self.content == Some(child) {
            self.content = None;
        }
    }

    fn pointer_children(&self) -> Vec<NodeId> {
        Vec::new()
    }

    fn live_children(&self) -> Vec<NodeId> {
        match self.open {
            true => self.children(),
            false => Vec::new(),
        }
    }

    fn kind(&self) -> &'static str {
        "overlay"
    }

    fn detail(&self) -> Option<String> {
        Some(if self.open { "open" } else { "closed" }.to_owned())
    }

    fn properties(&self) -> Vec<(&'static str, String)> {
        vec![
            ("open", self.open.to_string()),
            ("policy", format!("{:?}", self.policy)),
            ("level", format!("{:?}", self.level)),
            ("placement", format!("{:?}", self.placement)),
            (
                "anchor",
                match &self.anchor {
                    OverlayAnchor::Node(_) => "node".to_owned(),
                    OverlayAnchor::Pinned { rect, .. } => format!(
                        "pinned to a node from {}, {} to {}, {}",
                        rect.min.x, rect.min.y, rect.max.x, rect.max.y
                    ),
                    OverlayAnchor::Point(point) => format!("{}, {}", point.x, point.y),
                    OverlayAnchor::Rect(rect) => format!(
                        "{}, {} to {}, {}",
                        rect.min.x, rect.min.y, rect.max.x, rect.max.y
                    ),
                },
            ),
            ("traps focus", self.traps_focus.to_string()),
            (
                "window",
                self.window
                    .as_ref()
                    .map_or_else(|| "none".to_owned(), |window| format!("{:?}", window.id)),
            ),
            ("dismissable", (!self.on_dismiss.is_empty()).to_string()),
        ]
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Document {
    fn overlay_anchor_moved(&self, overlay: NodeOf<OverlayNode>, out: &Rects) -> bool {
        out.contains_key(&overlay.id())
            && !self.is_culled(overlay.id())
            && self.arena.get_as(overlay).anchor_moved(out)
    }

    pub(crate) fn relay_moved_overlays(&mut self) -> bool {
        let rects = Rc::clone(&self.rects);
        let moved: Vec<NodeOf<OverlayNode>> = self
            .open_overlays()
            .into_iter()
            .filter(|overlay| self.overlay_anchor_moved(*overlay, &rects))
            .collect();
        for overlay in &moved {
            self.arena.invalidate_node(*overlay);
        }
        !moved.is_empty()
    }

    pub(crate) fn place_moved_overlays(&mut self, ctx: &crate::context::Context, out: &Rects) {
        for overlay in self.open_overlays() {
            if !self.overlay_anchor_moved(overlay, out) {
                continue;
            }
            let window = self.window_of(overlay.id());
            let painter = ctx.window_painter(window);
            let Some(content) = self.arena.get_as(overlay).content else {
                continue;
            };
            let viewport = self.window_viewport(window);
            let mut element = self.arena.take(overlay);
            let outer = self.enter_measure(overlay.id());
            let held = self.swap_window_viewport(viewport);
            if let Some(node) = element.as_any_mut().downcast_mut::<OverlayNode>() {
                node.place_content(self, &painter, content, out);
            }
            self.swap_window_viewport(held);
            self.leave_measure(outer);
            self.arena.put_back(overlay, element);
            self.forget_placing();
        }
    }

    pub fn create_overlay(
        &mut self,
        anchor: OverlayAnchor,
        placement: Placement,
    ) -> NodeOf<OverlayNode> {
        let scrim = self.create_frame();
        self.arena
            .insert(OverlayNode::new(scrim, anchor, placement))
    }

    pub fn set_overlay_content(&mut self, overlay: NodeOf<OverlayNode>, content: Option<NodeId>) {
        if self.arena.get_as::<OverlayNode>(overlay).content != content {
            self.arena.get_mut_as::<OverlayNode>(overlay).content = content;
            self.layers_changed();
        }
    }

    pub fn set_overlay_window(&mut self, overlay: NodeOf<OverlayNode>, window: Option<WindowSpec>) {
        if self.arena.get_as::<OverlayNode>(overlay).window == window {
            return;
        }
        let reshaped = self
            .arena
            .get_as::<OverlayNode>(overlay)
            .window
            .as_ref()
            .map(|held| held.id)
            != window.as_ref().map(|window| window.id);
        if let Some(window) = &window {
            self.windows.note(window.id, overlay);
        }
        self.arena.get_mut_as::<OverlayNode>(overlay).window = window;
        if reshaped {
            self.layers_changed();
        }
    }

    pub fn content_of_overlay(&self, node: NodeId) -> Option<NodeId> {
        let overlay = self.arena.kind_of::<OverlayNode>(node)?;
        self.overlay_content(overlay)
    }

    pub fn overlay_window(&self, overlay: NodeOf<OverlayNode>) -> Option<&WindowSpec> {
        self.arena.get_as::<OverlayNode>(overlay).window.as_ref()
    }

    pub fn set_overlay_anchor(&mut self, overlay: NodeOf<OverlayNode>, anchor: OverlayAnchor) {
        if self.arena.get_as::<OverlayNode>(overlay).anchor != anchor {
            let node = self.arena.get_mut_as::<OverlayNode>(overlay);
            node.anchor = anchor;
            node.pinned = None;
        }
    }

    pub fn set_overlay_pointer(&mut self, overlay: NodeOf<OverlayNode>, pointer: Option<NodeRef>) {
        if self.arena.get_as::<OverlayNode>(overlay).pointer != pointer {
            self.arena.get_mut_as::<OverlayNode>(overlay).pointer = pointer;
        }
    }

    pub fn set_overlay_beak(&mut self, overlay: NodeOf<OverlayNode>, reach: f32) {
        match self.arena.get_as::<OverlayNode>(overlay).beak {
            Some(beak) => {
                if self.arena.get_as::<BeakNode>(beak).reach != reach {
                    self.arena.paint_mut_as::<BeakNode>(beak).reach = reach;
                }
            }
            None if reach > 0.0 => {
                let beak = self.arena.insert(BeakNode {
                    reach,
                    style: FrameStyle::default(),
                    pointing: None,
                });
                self.arena.get_mut_as::<OverlayNode>(overlay).beak = Some(beak);
            }
            None => {}
        }
    }

    pub fn overlay_beak(&self, overlay: NodeOf<OverlayNode>) -> Option<NodeId> {
        self.arena
            .get_as::<OverlayNode>(overlay)
            .beak
            .map(NodeOf::id)
    }

    pub fn set_overlay_scrim(&mut self, overlay: NodeOf<OverlayNode>, color: impl Into<Color>) {
        let color = color.into();
        let node = self.arena.get_as::<OverlayNode>(overlay);
        let (scrim, dims) = (node.scrim, !color.is_clear());
        let (held, dimmed) = (node.scrim_color, node.dims);
        if held != color {
            self.arena.get_mut_as::<OverlayNode>(overlay).scrim_color = color;
        }
        if dimmed != dims {
            self.arena.get_mut_as::<OverlayNode>(overlay).dims = dims;
            self.layers_changed();
        }
        self.set_frame_color(scrim, color);
    }

    pub fn set_overlay_placement(&mut self, overlay: NodeOf<OverlayNode>, placement: Placement) {
        if self.arena.get_as::<OverlayNode>(overlay).placement != placement {
            self.arena.get_mut_as::<OverlayNode>(overlay).placement = placement;
        }
    }

    pub fn set_overlay_traps_focus(&mut self, overlay: NodeOf<OverlayNode>, traps_focus: bool) {
        if self.arena.get_as::<OverlayNode>(overlay).traps_focus != traps_focus {
            self.arena.touch_mut_as::<OverlayNode>(overlay).traps_focus = traps_focus;
            self.layers_changed();
        }
    }

    pub fn set_overlay_policy(&mut self, overlay: NodeOf<OverlayNode>, policy: LayerPolicy) {
        if self.arena.get_as::<OverlayNode>(overlay).policy != policy {
            self.arena.get_mut_as::<OverlayNode>(overlay).policy = policy;
            self.layers_changed();
        }
    }

    pub fn set_overlay_shown_in(&mut self, overlay: NodeOf<OverlayNode>, window: Option<WindowId>) {
        if self.arena.get_as::<OverlayNode>(overlay).shown_in != window {
            self.arena.get_mut_as::<OverlayNode>(overlay).shown_in = window;
            self.layers_changed();
        }
    }

    pub fn set_overlay_level(&mut self, overlay: NodeOf<OverlayNode>, level: LayerLevel) {
        if self.arena.get_as::<OverlayNode>(overlay).level != level {
            self.arena.get_mut_as::<OverlayNode>(overlay).level = level;
            self.layers_changed();
        }
    }

    pub fn set_overlay_trigger(&mut self, overlay: NodeOf<OverlayNode>, trigger: Option<NodeRef>) {
        self.arena.get_mut_as::<OverlayNode>(overlay).trigger = trigger;
    }

    pub fn set_overlay_on_dismiss(
        &mut self,
        overlay: NodeOf<OverlayNode>,
        on_dismiss: Callback<Dismiss>,
    ) {
        self.arena.touch_mut_as::<OverlayNode>(overlay).on_dismiss = on_dismiss;
        self.layer_state.changed();
    }

    pub fn is_overlay(&self, node: NodeId) -> bool {
        self.arena.kind_of::<OverlayNode>(node).is_some()
    }

    pub fn is_overlay_open(&self, overlay: NodeId) -> bool {
        self.arena
            .kind_of::<OverlayNode>(overlay)
            .is_some_and(|overlay| self.arena.get_as(overlay).open)
    }

    pub fn overlay_scrim(&self, overlay: NodeOf<OverlayNode>) -> NodeId {
        self.arena.get_as::<OverlayNode>(overlay).scrim.id()
    }

    pub fn overlay_content(&self, overlay: NodeOf<OverlayNode>) -> Option<NodeId> {
        self.arena.get_as::<OverlayNode>(overlay).content
    }

    pub(crate) fn on_overlay_trigger(&self, overlay: NodeOf<OverlayNode>, pos: Pos2) -> bool {
        self.arena
            .get_as::<OverlayNode>(overlay)
            .trigger
            .as_ref()
            .and_then(NodeRef::try_get)
            .and_then(|trigger| self.node_rect(trigger))
            .is_some_and(|rect| rect.contains_half_open(pos))
    }
}
