use std::any::Any;
use std::rc::Rc;

use crate::color::Color32;
use crate::geometry::{Pos2, Rect, Vec2, pos2, vec2};
use crate::painter::Painter;

use crate::base::frame::{FrameNode, PointerSide, Pointing};
use crate::callback::{Callback, NodeRef};
use crate::document::Document;
use crate::layer::{Dismiss, LayerLevel, LayerPolicy};
use crate::node::{Element, NodeId, NodeOf, Rects};
use crate::window::{WindowId, WindowPlacement, WindowReport};

#[derive(Clone, PartialEq)]
pub enum OverlayAnchor {
    Node(NodeRef),
    Point(Pos2),
    Rect(Rect),
    Pinned { node: NodeRef, rect: Rect },
}

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

impl beui_tree::reactive::IntoProp<OverlayAnchor> for &NodeRef {
    fn into_prop(self) -> beui_tree::reactive::Prop<OverlayAnchor> {
        beui_tree::reactive::Prop::Static(OverlayAnchor::Node(*self))
    }
}

impl beui_tree::reactive::IntoProp<OverlayAnchor> for NodeRef {
    fn into_prop(self) -> beui_tree::reactive::Prop<OverlayAnchor> {
        beui_tree::reactive::Prop::Static(OverlayAnchor::Node(self))
    }
}

impl Default for OverlayAnchor {
    fn default() -> Self {
        OverlayAnchor::Point(Pos2::ZERO)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Placement {
    At,
    Above,
    Over(u16),
    Around,
    BelowStart,
    RightStart,
    Center,
    Fill,
    FillScreen,
    InsideTop,
    InsideTopEnd,
    InsideBottom,
    InsideBottomEnd,
    CenterOn,
    HangFrom(u16),
    Popover { gap: u16, margin: u16 },
}

#[derive(Clone)]
pub struct WindowSpec {
    pub id: WindowId,
    pub size: Vec2,
    pub placement: WindowPlacement,
    pub title: Option<String>,
    pub on_report: Callback<WindowReport>,
}

impl PartialEq for WindowSpec {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.size == other.size
            && self.placement == other.placement
            && self.title == other.title
    }
}

pub struct OverlayNode {
    pub(crate) content: Option<NodeId>,
    pub(crate) window: Option<WindowSpec>,
    scrim: NodeOf<FrameNode>,
    dims: bool,
    pub(crate) open: bool,
    pub(crate) anchor: OverlayAnchor,
    anchored: Option<Rect>,
    pinned: Option<Pin>,
    pointer: Option<NodeRef>,
    placement: Placement,
    pub(crate) policy: LayerPolicy,
    pub(crate) level: LayerLevel,
    pub(crate) traps_focus: bool,
    pub(crate) trigger: Option<NodeRef>,
    pub(crate) screen: Option<String>,
    pub(crate) on_dismiss: Callback<Dismiss>,
}

impl OverlayNode {
    fn new(scrim: NodeOf<FrameNode>, anchor: OverlayAnchor, placement: Placement) -> Self {
        Self {
            content: None,
            window: None,
            scrim,
            dims: false,
            open: false,
            anchor,
            anchored: None,
            pinned: None,
            pointer: None,
            placement,
            policy: LayerPolicy::BlocksAll,
            level: LayerLevel::Normal,
            traps_focus: true,
            trigger: None,
            screen: None,
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

    fn area(&self, doc: &Document, viewport: Rect, anchored: Option<Rect>) -> Rect {
        if doc.in_window_viewport() {
            return viewport;
        }
        let opened = || {
            self.screen
                .as_deref()
                .and_then(|id| doc.screen_named(id))
                .or_else(|| doc.screens().into_iter().next())
        };
        let screen = match (self.placement, &self.anchor) {
            (Placement::Fill | Placement::At | Placement::Around, _) => None,
            (Placement::Center | Placement::FillScreen, _) => opened(),
            (_, OverlayAnchor::Node(_) | OverlayAnchor::Pinned { .. }) => {
                anchored.map_or_else(opened, |rect| doc.screen_under(rect))
            }
            (_, OverlayAnchor::Point(pos)) => doc.screen_at(*pos),
            (_, OverlayAnchor::Rect(rect)) => doc.screen_under(*rect),
        };
        screen.map_or(viewport, |screen| screen.rect)
    }
}

fn resolve_rect(
    viewport: Rect,
    anchor_rect: Rect,
    placement: Placement,
    content_size: Vec2,
) -> Rect {
    if placement == Placement::At {
        return Rect::from_min_size(anchor_rect.min, content_size);
    }
    if let Placement::Over(inset) = placement {
        let inset = f32::from(inset);
        let right = (viewport.right() - content_size.x).max(viewport.left());
        let bottom = (viewport.bottom() - content_size.y).max(viewport.top());
        let origin = pos2(
            (anchor_rect.left() - inset).min(right).max(viewport.left()),
            (anchor_rect.top() - inset).min(bottom).max(viewport.top()),
        );
        return Rect::from_min_size(origin, content_size);
    }
    if placement == Placement::Around {
        return Rect::from_center_size(anchor_rect.center(), content_size);
    }
    if placement == Placement::Above {
        let right = (viewport.right() - content_size.x).max(viewport.left());
        let bottom = (viewport.bottom() - content_size.y).max(viewport.top());
        let y = match anchor_rect.top() - content_size.y >= viewport.top() {
            true => anchor_rect.top() - content_size.y,
            false => anchor_rect.bottom(),
        };
        let origin = pos2(
            (anchor_rect.center().x - content_size.x / 2.0)
                .min(right)
                .max(viewport.left()),
            y.min(bottom).max(viewport.top()),
        );
        return Rect::from_min_size(origin, content_size);
    }
    if placement == Placement::CenterOn {
        let size = content_size.min(viewport.size()).max(Vec2::ZERO);
        let centred = Rect::from_center_size(anchor_rect.center(), size);
        let x = centred
            .left()
            .min(viewport.right() - size.x)
            .max(viewport.left());
        let y = centred
            .top()
            .min(viewport.bottom() - size.y)
            .max(viewport.top());
        return Rect::from_min_size(pos2(x, y), size);
    }
    if let Placement::HangFrom(gap) = placement {
        let gap = f32::from(gap);
        let room = viewport.shrink(gap);
        let size = content_size.min(room.size()).max(Vec2::ZERO);
        let x = (anchor_rect.center().x - size.x / 2.0)
            .min(room.right() - size.x)
            .max(room.left());
        let y = (anchor_rect.top() + gap)
            .min(room.bottom() - size.y)
            .max(room.top());
        return Rect::from_min_size(pos2(x, y), size);
    }
    if let Placement::Popover { gap, margin } = placement {
        let gap = f32::from(gap);
        let centre = anchor_rect.center().x;
        let reach = Rect::from_min_max(
            pos2(centre - content_size.x / 2.0, anchor_rect.top() - gap),
            pos2(centre + content_size.x / 2.0, anchor_rect.bottom() + gap),
        );
        let room = viewport.shrink(f32::from(margin));
        return resolve_rect(room, reach, Placement::BelowStart, content_size);
    }
    if let Placement::Fill | Placement::FillScreen = placement {
        return viewport;
    }
    if placement == Placement::Center {
        let content_size = content_size.min(viewport.size()).max(Vec2::ZERO);
        let origin = pos2(
            viewport.left() + (viewport.width() - content_size.x) / 2.0,
            viewport.top() + (viewport.height() - content_size.y) / 2.0,
        );
        return Rect::from_min_size(
            pos2(origin.x.max(viewport.left()), origin.y.max(viewport.top())),
            content_size,
        );
    }
    if let Placement::InsideTopEnd | Placement::InsideBottomEnd = placement {
        let y = match placement {
            Placement::InsideTopEnd => anchor_rect.top(),
            _ => (anchor_rect.bottom() - content_size.y).max(anchor_rect.top()),
        };
        let origin = pos2(
            (anchor_rect.right() - content_size.x).max(anchor_rect.left()),
            y,
        );
        return Rect::from_min_size(
            pos2(origin.x.max(viewport.left()), origin.y.max(viewport.top())),
            content_size,
        );
    }
    if let Placement::InsideTop | Placement::InsideBottom = placement {
        let y = match placement {
            Placement::InsideTop => anchor_rect.top(),
            _ => anchor_rect.bottom() - content_size.y,
        };
        let origin = pos2(
            anchor_rect.center().x - content_size.x / 2.0,
            y.clamp(
                anchor_rect.top(),
                (anchor_rect.bottom() - content_size.y).max(anchor_rect.top()),
            ),
        );
        return Rect::from_min_size(
            pos2(origin.x.max(viewport.left()), origin.y.max(viewport.top())),
            content_size,
        );
    }
    let mut content_size = content_size.min(viewport.size());
    if placement == Placement::BelowStart {
        let below = viewport.bottom() - anchor_rect.bottom();
        let above = anchor_rect.top() - viewport.top();
        if content_size.y > below && content_size.y > above && below.max(above) > 0.0 {
            content_size.y = below.max(above);
            let top = match below >= above {
                true => anchor_rect.bottom(),
                false => anchor_rect.top() - content_size.y,
            };
            let left = anchor_rect
                .left()
                .min(viewport.right() - content_size.x)
                .max(viewport.left());
            return Rect::from_min_size(pos2(left, top), content_size);
        }
    }
    let mut origin = match placement {
        Placement::BelowStart => pos2(anchor_rect.left(), anchor_rect.bottom()),
        Placement::RightStart => pos2(anchor_rect.right(), anchor_rect.top()),
        Placement::At
        | Placement::Above
        | Placement::Over(_)
        | Placement::Around
        | Placement::Center
        | Placement::Fill
        | Placement::FillScreen
        | Placement::InsideTop
        | Placement::InsideTopEnd
        | Placement::InsideBottom
        | Placement::InsideBottomEnd
        | Placement::CenterOn
        | Placement::HangFrom(_)
        | Placement::Popover { .. } => {
            unreachable!("a centred or pinned overlay is placed above")
        }
    };
    if origin.x + content_size.x > viewport.right() {
        origin.x = match placement {
            Placement::RightStart => anchor_rect.left() - content_size.x,
            _ => (viewport.right() - content_size.x).max(viewport.left()),
        };
    }
    if origin.y + content_size.y > viewport.bottom() {
        origin.y = match placement {
            Placement::BelowStart => anchor_rect.top() - content_size.y,
            _ => (viewport.bottom() - content_size.y).max(viewport.top()),
        };
    }
    origin.x = origin.x.max(viewport.left());
    origin.y = origin.y.max(viewport.top());
    Rect::from_min_size(origin, content_size)
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
        let area = self.area(doc, viewport, target);
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
        let rect = resolve_rect(area, anchor_rect, self.placement, content_size);
        crate::layout::layout(doc, painter, content, rect, out);
        self.stand_off(doc, painter, content, rect, anchor_rect, out);
        self.point_at(doc, anchor_rect, out);
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

    fn point_at(&self, doc: &mut Document, anchor: Rect, out: &Rects) {
        let Some(frame) = self.pointer_frame(doc) else {
            return;
        };
        let pointing = out.get(&frame.id()).and_then(|rect| {
            if !matches!(self.placement, Placement::Popover { .. }) {
                return None;
            }
            let (lowest, highest) = doc.arena.get_as::<FrameNode>(frame).pointer_room(rect);
            let centre = anchor.center().x;
            if centre < lowest || centre > highest {
                return None;
            }
            let side = match (rect.top() >= anchor.bottom(), rect.bottom() <= anchor.top()) {
                (true, _) => PointerSide::Top,
                (false, true) => PointerSide::Bottom,
                (false, false) => return None,
            };
            Some(Pointing {
                side,
                at: centre - rect.left(),
            })
        });
        doc.set_frame_pointing(frame, pointing);
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
        let real = self.window.as_ref().filter(|window| doc.hosts_window(window));
        let painter = &painter
            .ctx()
            .window_painter(real.map_or_else(|| doc.viewport_window(), |window| window.id));
        if let Some(window) = real {
            let size = painter.ctx().window_size(window.id).unwrap_or(window.size);
            let rect = Rect::from_min_size(Pos2::ZERO, size);
            let outer = doc.swap_window_viewport(Some((window.id, rect)));
            crate::layout::layout(doc, painter, content, rect, out);
            doc.swap_window_viewport(outer);
            return;
        }
        let viewport = doc.viewport_rect();
        if self.policy == LayerPolicy::BlocksAll {
            crate::layout::layout(doc, painter, self.scrim.id(), viewport, out);
        }
        self.place_content(doc, painter, content, out);
    }

    fn paint(&self, _doc: &Document, _painter: &Painter, _rects: &Rects, _rect: Rect) {}

    fn paints(&self) -> bool {
        false
    }

    fn children(&self) -> Vec<NodeId> {
        let mut children = vec![self.scrim.id()];
        children.extend(self.content);
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

    pub fn set_overlay_scrim(&mut self, overlay: NodeOf<OverlayNode>, color: Color32) {
        let node = self.arena.get_as::<OverlayNode>(overlay);
        let (scrim, dims) = (node.scrim, color.alpha() > 0);
        if node.dims != dims {
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

    pub(crate) fn opening_screen(&self, overlay: NodeOf<OverlayNode>) -> Option<String> {
        let rect_of = |node: Option<NodeId>| node.and_then(|node| self.node_rect(node));
        let trigger = self
            .arena
            .get_as::<OverlayNode>(overlay)
            .trigger
            .as_ref()
            .and_then(NodeRef::try_get);
        let pressed = match self.acted {
            crate::document::Acted::Pointer(pos) => Some(Rect::from_min_size(pos, Vec2::ZERO)),
            crate::document::Acted::Keys | crate::document::Acted::Nothing => None,
        };
        let pointer = self
            .last_pointer
            .map(|sample| Rect::from_min_size(sample.pos, Vec2::ZERO));
        let rect = rect_of(trigger)
            .or(pressed)
            .or_else(|| rect_of(self.focused))
            .or(pointer)?;
        Some(self.screen_under(rect)?.id)
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
