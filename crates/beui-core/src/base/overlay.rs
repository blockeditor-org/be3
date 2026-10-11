use std::any::Any;
use std::rc::Rc;

use crate::color::Color32;
use crate::geometry::{Pos2, Rect, Vec2, pos2};
use crate::painter::Painter;

use crate::base::frame::FrameNode;
use crate::callback::{Callback, NodeRef};
use crate::document::Document;
use crate::layer::{Dismiss, LayerLevel, LayerPolicy};
use crate::node::{Element, NodeId, NodeOf, Rects};

#[derive(Clone, PartialEq)]
pub enum OverlayAnchor {
    Node(NodeRef),
    Point(Pos2),
    Rect(Rect),
}

impl beui_tree::reactive::IntoProp<OverlayAnchor> for &NodeRef {
    fn into_prop(self) -> beui_tree::reactive::Prop<OverlayAnchor> {
        beui_tree::reactive::Prop::Static(OverlayAnchor::Node(self.clone()))
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
}

pub struct OverlayNode {
    pub(crate) content: Option<NodeId>,
    scrim: NodeOf<FrameNode>,
    pub(crate) open: bool,
    pub(crate) anchor: OverlayAnchor,
    anchored: Option<Rect>,
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
            scrim,
            open: false,
            anchor,
            anchored: None,
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
            OverlayAnchor::Node(node) => node.try_get(),
            OverlayAnchor::Point(_) | OverlayAnchor::Rect(_) => None,
        }
    }

    fn area(&self, doc: &Document, viewport: Rect, anchored: Option<Rect>) -> Rect {
        let opened = || {
            self.screen
                .as_deref()
                .and_then(|id| doc.screen_named(id))
                .or_else(|| doc.screens().into_iter().next())
        };
        let screen = match (self.placement, &self.anchor) {
            (Placement::Fill | Placement::At | Placement::Around, _) => None,
            (Placement::Center | Placement::FillScreen, _) => opened(),
            (_, OverlayAnchor::Node(_)) => {
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
    if let Placement::Fill | Placement::FillScreen = placement {
        return viewport;
    }
    if placement == Placement::Center {
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
        | Placement::Over(_)
        | Placement::Around
        | Placement::Center
        | Placement::Fill
        | Placement::FillScreen
        | Placement::InsideTop
        | Placement::InsideTopEnd
        | Placement::InsideBottom
        | Placement::InsideBottomEnd
        | Placement::CenterOn => {
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


impl Element for OverlayNode {
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
        let painter = &painter.ctx().painter();
        let viewport = doc.viewport_rect();
        if self.policy == LayerPolicy::BlocksAll {
            crate::layout::layout(doc, painter, self.scrim.id(), viewport, out);
        }
        let anchored = self.anchor_node().and_then(|id| out.get(&id));
        self.anchored = anchored;
        let area = self.area(doc, viewport, anchored);
        let content_size = crate::layout::measure(doc, painter, content, area.size());
        let anchor_rect = match &self.anchor {
            OverlayAnchor::Node(_) => anchored.unwrap_or(area),
            OverlayAnchor::Point(pos) => Rect::from_min_size(*pos, Vec2::ZERO),
            OverlayAnchor::Rect(rect) => *rect,
        };
        let rect = resolve_rect(area, anchor_rect, self.placement, content_size);
        crate::layout::layout(doc, painter, content, rect, out);
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
                    OverlayAnchor::Point(point) => format!("{}, {}", point.x, point.y),
                    OverlayAnchor::Rect(rect) => format!(
                        "{}, {} to {}, {}",
                        rect.min.x, rect.min.y, rect.max.x, rect.max.y
                    ),
                },
            ),
            ("traps focus", self.traps_focus.to_string()),
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
    pub(crate) fn relay_moved_overlays(&mut self) -> bool {
        let rects = Rc::clone(&self.rects);
        let moved: Vec<NodeOf<OverlayNode>> = self
            .open_overlays()
            .into_iter()
            .filter(|overlay| {
                let node = self.arena.get_as(*overlay);
                node.anchor_node()
                    .and_then(|id| rects.get(&id))
                    .is_some_and(|rect| node.anchored != Some(rect))
            })
            .collect();
        for overlay in &moved {
            self.arena.invalidate_node(*overlay);
        }
        !moved.is_empty()
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

    pub fn set_overlay_content(&mut self, overlay: NodeOf<OverlayNode>, content: NodeId) {
        self.arena.get_mut_as::<OverlayNode>(overlay).content = Some(content);
    }

    pub fn set_overlay_anchor(&mut self, overlay: NodeOf<OverlayNode>, anchor: OverlayAnchor) {
        self.arena.get_mut_as::<OverlayNode>(overlay).anchor = anchor;
    }

    pub fn set_overlay_scrim(&mut self, overlay: NodeOf<OverlayNode>, color: Color32) {
        let scrim = self.arena.get_as::<OverlayNode>(overlay).scrim;
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
