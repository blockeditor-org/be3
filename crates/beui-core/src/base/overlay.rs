use std::any::Any;
use std::cell::Cell;
use std::rc::Rc;

use crate::color::Color32;
use crate::geometry::{Pos2, Rect, Vec2, pos2};
use crate::input::{CursorIcon, PointerPress};
use crate::painter::Painter;

use crate::base::frame::FrameNode;
use crate::base::interactive::InteractiveNode;
use crate::callback::{Callback, NodeRef};
use crate::current::with_document;
use crate::document::{Acted, Document};
use crate::node::{ClickHandler, Element, InteractInput, NodeId, NodeOf, Rects};

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
}

#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub enum OverlayMode {
    #[default]
    Modal,
    Floating,
    Passive,
}

impl OverlayMode {
    fn stacked(self) -> bool {
        self == OverlayMode::Modal
    }
}

pub struct OverlayNode {
    content: Option<NodeId>,
    scrim: NodeId,
    dim: NodeOf<FrameNode>,
    open: bool,
    anchor: OverlayAnchor,
    anchored: Option<Rect>,
    placement: Placement,
    traps_focus: bool,
    light: bool,
    trigger: Option<NodeRef>,
    screen: Option<String>,
    mode: OverlayMode,
    locks: bool,
    on_dismiss: Option<ClickHandler>,
}

impl OverlayNode {
    fn new(
        scrim: NodeId,
        dim: NodeOf<FrameNode>,
        anchor: OverlayAnchor,
        placement: Placement,
    ) -> Self {
        Self {
            content: None,
            scrim,
            dim,
            open: false,
            anchor,
            anchored: None,
            placement,
            traps_focus: true,
            light: false,
            trigger: None,
            screen: None,
            mode: OverlayMode::Modal,
            locks: false,
            on_dismiss: None,
        }
    }

    pub fn is_open(&self) -> bool {
        self.open
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
        | Placement::InsideBottomEnd => {
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
        crate::layout::layout(doc, painter, self.scrim, viewport, out);
        let anchored = match &self.anchor {
            OverlayAnchor::Node(node) => node.try_get().and_then(|id| out.get(&id)),
            OverlayAnchor::Point(_) | OverlayAnchor::Rect(_) => None,
        };
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

    fn interact(
        &mut self,
        _doc: &mut Document,
        _painter: &Painter,
        _input: &InteractInput,
        _id: NodeId,
        _rect: Rect,
        _focus_target: &mut Option<NodeId>,
        children: &mut Vec<NodeId>,
    ) {
        if !self.open || self.mode != OverlayMode::Modal {
            return;
        }
        if !self.light {
            children.push(self.scrim);
        }
        children.extend(self.content);
    }

    fn children(&self) -> Vec<NodeId> {
        let mut children = vec![self.scrim];
        children.extend(self.content);
        children
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
            ("mode", format!("{:?}", self.mode)),
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
            ("light dismiss", self.light.to_string()),
            ("locks", self.locks.to_string()),
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
            .overlay_stack
            .iter()
            .chain(self.passive_overlays.iter())
            .copied()
            .filter(|overlay| {
                if !self.contains(*overlay) {
                    return false;
                }
                let node = self.arena.get_as(*overlay);
                let OverlayAnchor::Node(anchor) = &node.anchor else {
                    return false;
                };
                node.open
                    && anchor
                        .try_get()
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
        let overlay_cell: Rc<Cell<Option<NodeOf<OverlayNode>>>> = Rc::new(Cell::new(None));
        let press_cell = overlay_cell.clone();
        let tap_cell = overlay_cell.clone();
        let scrim = self.create_interactive(false);
        self.set_interactive_cursor(scrim, Some(CursorIcon::Default));
        let dim = self.create_frame();
        self.set_interactive_child(scrim, dim.id());
        let node = self.arena.touch_mut_as::<InteractiveNode>(scrim);
        node.on_press = Callback::new(move |press: PointerPress| {
            if press.touch {
                return;
            }
            let id = press_cell.get().expect("overlay not yet initialized");
            with_document(|document| document.dismiss_overlay_if_outside(id, press.pos));
        });
        node.on_click_at = Callback::new(move |press: PointerPress| {
            if !press.touch {
                return;
            }
            let id = tap_cell.get().expect("overlay not yet initialized");
            with_document(|document| document.dismiss_overlay_if_outside(id, press.pos));
        });
        let id = self
            .arena
            .insert(OverlayNode::new(scrim.id(), dim, anchor, placement));
        overlay_cell.set(Some(id));
        id
    }

    pub fn set_overlay_content(&mut self, overlay: NodeOf<OverlayNode>, content: NodeId) {
        self.arena.get_mut_as::<OverlayNode>(overlay).content = Some(content);
    }

    pub fn set_overlay_anchor(&mut self, overlay: NodeOf<OverlayNode>, anchor: OverlayAnchor) {
        self.arena.get_mut_as::<OverlayNode>(overlay).anchor = anchor;
    }

    pub fn set_overlay_scrim(&mut self, overlay: NodeOf<OverlayNode>, color: Color32) {
        let dim = self.arena.get_as::<OverlayNode>(overlay).dim;
        self.set_frame_color(dim, color);
    }

    pub fn set_overlay_placement(&mut self, overlay: NodeOf<OverlayNode>, placement: Placement) {
        if self.arena.get_as::<OverlayNode>(overlay).placement != placement {
            self.arena.get_mut_as::<OverlayNode>(overlay).placement = placement;
        }
    }

    pub fn set_overlay_traps_focus(&mut self, overlay: NodeOf<OverlayNode>, traps_focus: bool) {
        if self.arena.get_as::<OverlayNode>(overlay).traps_focus != traps_focus {
            self.arena.touch_mut_as::<OverlayNode>(overlay).traps_focus = traps_focus;
        }
    }

    pub fn set_overlay_mode(&mut self, overlay: NodeOf<OverlayNode>, mode: OverlayMode) {
        if self.arena.get_as::<OverlayNode>(overlay).mode == mode {
            return;
        }
        self.arena.get_mut_as::<OverlayNode>(overlay).mode = mode;
        if self.arena.get_as::<OverlayNode>(overlay).open {
            self.close_overlay(overlay);
            self.open_overlay(overlay);
        }
    }

    pub fn overlay_is_floating(&self, overlay: NodeOf<OverlayNode>) -> bool {
        self.contains(overlay)
            && self.arena.get_as::<OverlayNode>(overlay).mode == OverlayMode::Floating
            && self.arena.get_as::<OverlayNode>(overlay).open
    }

    pub fn floating_overlays(&self) -> Vec<NodeOf<OverlayNode>> {
        self.passive_overlays
            .iter()
            .copied()
            .filter(|overlay| self.overlay_is_floating(*overlay))
            .collect()
    }

    pub fn overlays_bottom_up(&self) -> Vec<NodeOf<OverlayNode>> {
        let floating = self.floating_overlays();
        let passive: Vec<NodeOf<OverlayNode>> = self
            .passive_overlays
            .iter()
            .filter(|overlay| !floating.contains(overlay))
            .copied()
            .collect();
        let Some(lock) = self.lock() else {
            return floating
                .iter()
                .chain(self.overlay_stack.iter())
                .chain(passive.iter())
                .copied()
                .collect();
        };
        let at = self
            .overlay_stack
            .iter()
            .position(|overlay| *overlay == lock)
            .unwrap_or(self.overlay_stack.len());
        let inside = self.overlays_within(lock, &passive);
        let outside: Vec<_> = passive
            .into_iter()
            .filter(|overlay| !inside.contains(overlay))
            .collect();
        floating
            .iter()
            .chain(self.overlay_stack[..at].iter())
            .chain(outside.iter())
            .chain(self.overlay_stack[at..].iter())
            .chain(inside.iter())
            .copied()
            .collect()
    }

    pub fn set_overlay_locks(&mut self, overlay: NodeOf<OverlayNode>, locks: bool) {
        if self.arena.get_as::<OverlayNode>(overlay).locks == locks {
            return;
        }
        self.arena.touch_mut_as::<OverlayNode>(overlay).locks = locks;
        if self.arena.get_as::<OverlayNode>(overlay).open {
            self.close_overlay(overlay);
            self.open_overlay(overlay);
        }
    }

    pub(crate) fn lock(&self) -> Option<NodeOf<OverlayNode>> {
        self.overlay_stack.iter().copied().find(|overlay| {
            self.contains(*overlay) && self.arena.get_as::<OverlayNode>(*overlay).locks
        })
    }

    pub fn locked(&self) -> bool {
        self.lock().is_some()
    }

    pub(crate) fn is_within(&self, node: NodeId, ancestor: NodeId) -> bool {
        let mut pending = vec![ancestor];
        while let Some(id) = pending.pop() {
            if id == node {
                return true;
            }
            if self.contains(id) {
                pending.extend(self.arena.get(id).children());
            }
        }
        false
    }

    pub fn pointer_layers(&self, root: NodeId) -> Vec<NodeId> {
        match self.overlay_stack.is_empty() {
            true => self
                .floating_overlays()
                .into_iter()
                .rev()
                .map(NodeOf::id)
                .chain([root])
                .collect(),
            false => self
                .overlay_stack
                .iter()
                .rev()
                .map(|overlay| overlay.id())
                .collect(),
        }
    }

    pub fn floating_covers(&self, pos: Pos2) -> bool {
        if self.modal_open() {
            return false;
        }
        self.floating_overlays().into_iter().any(|overlay| {
            self.overlay_content(overlay)
                .and_then(|content| self.node_rect(content))
                .is_some_and(|rect| rect.contains_half_open(pos))
        })
    }

    pub fn raise_overlay(&mut self, overlay: NodeId) {
        let Some(index) = self
            .passive_overlays
            .iter()
            .position(|id| id.id() == overlay)
        else {
            return;
        };
        if index + 1 == self.passive_overlays.len() {
            return;
        }
        let raised = self.passive_overlays.remove(index);
        self.passive_overlays.push(raised);
        self.arena.invalidate_node(raised);
    }

    pub fn overlay_traps_focus(&self, overlay: NodeOf<OverlayNode>) -> bool {
        self.arena.get_as::<OverlayNode>(overlay).traps_focus
    }

    pub fn set_overlay_on_dismiss(
        &mut self,
        overlay: NodeOf<OverlayNode>,
        handler: impl FnMut() + 'static,
    ) {
        self.arena.touch_mut_as::<OverlayNode>(overlay).on_dismiss = Some(Box::new(handler));
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
        self.arena.get_as::<OverlayNode>(overlay).scrim
    }

    pub fn overlay_content(&self, overlay: NodeOf<OverlayNode>) -> Option<NodeId> {
        self.arena.get_as::<OverlayNode>(overlay).content
    }

    pub fn modal_open(&self) -> bool {
        !self.overlay_stack.is_empty()
    }

    pub fn floating_rects(&self) -> Vec<Rect> {
        self.floating_overlays()
            .into_iter()
            .filter_map(|overlay| self.overlay_content(overlay))
            .filter_map(|content| self.node_rect(content))
            .collect()
    }

    pub fn pointer_claimed(&self, pos: Pos2) -> bool {
        self.modal_open() || self.floating_covers(pos)
    }

    pub fn overlay_rects(&self) -> Vec<Rect> {
        self.overlay_stack
            .iter()
            .chain(self.passive_overlays.iter())
            .filter_map(|overlay| self.overlay_cover(*overlay))
            .collect()
    }

    pub fn overlay_cover(&self, overlay: NodeOf<OverlayNode>) -> Option<Rect> {
        self.cover_of(overlay, true)
    }

    pub fn overlay_occluder(&self, overlay: NodeOf<OverlayNode>) -> Option<Rect> {
        self.cover_of(overlay, false)
    }

    fn cover_of(&self, overlay: NodeOf<OverlayNode>, light_scrim: bool) -> Option<Rect> {
        let node = self.arena.get_as::<OverlayNode>(overlay);
        let content = self.node_rect(node.content?)?;
        let cover = match node.mode.stacked() && (light_scrim || !node.light) {
            true => self
                .node_rect(node.scrim)
                .map_or(content, |scrim| scrim.union(content)),
            false => content,
        };
        cover.is_positive().then_some(cover)
    }

    pub fn open_overlay(&mut self, overlay: NodeOf<OverlayNode>) {
        if self.arena.get_as::<OverlayNode>(overlay).open {
            return;
        }
        let screen = self.opening_screen(overlay);
        let node = self.arena.get_mut_as::<OverlayNode>(overlay);
        node.open = true;
        node.screen = screen;
        let below_lock = self
            .lock()
            .filter(|lock| !self.is_within(overlay.id(), lock.id()))
            .and_then(|lock| self.overlay_stack.iter().position(|open| *open == lock));
        match self.arena.get_as::<OverlayNode>(overlay).mode.stacked() {
            true => match below_lock {
                Some(at) => self.overlay_stack.insert(at, overlay),
                None => self.overlay_stack.push(overlay),
            },
            false => self.passive_overlays.push(overlay),
        }
        self.arena.invalidate_node(overlay);
    }

    fn opening_screen(&self, overlay: NodeOf<OverlayNode>) -> Option<String> {
        let rect_of = |node: Option<NodeId>| node.and_then(|node| self.node_rect(node));
        let trigger = self
            .arena
            .get_as::<OverlayNode>(overlay)
            .trigger
            .as_ref()
            .and_then(NodeRef::try_get);
        let pressed = match self.acted {
            Acted::Pointer(pos) => Some(Rect::from_min_size(pos, Vec2::ZERO)),
            Acted::Keys | Acted::Nothing => None,
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

    pub fn close_overlay(&mut self, overlay: NodeOf<OverlayNode>) {
        self.end_overlay(overlay, false);
    }

    pub fn dismiss_overlay(&mut self, overlay: NodeOf<OverlayNode>) {
        self.end_overlay(overlay, true);
    }

    fn end_overlay(&mut self, overlay: NodeOf<OverlayNode>, dismissed: bool) {
        if dismissed && self.contains(overlay) && self.arena.get_as::<OverlayNode>(overlay).locks {
            return;
        }
        let open: Vec<NodeOf<OverlayNode>> = self
            .overlay_stack
            .iter()
            .chain(self.passive_overlays.iter())
            .copied()
            .filter(|&id| id != overlay)
            .collect();
        if open.len() == self.overlay_stack.len() + self.passive_overlays.len() {
            return;
        }
        let mut nested = self.overlays_within(overlay, &open);
        if dismissed {
            nested.retain(|id| !self.arena.get_as::<OverlayNode>(*id).locks);
        }
        let closing = |id: &NodeOf<OverlayNode>| *id == overlay || nested.contains(id);
        self.overlay_stack.retain(|id| !closing(id));
        self.passive_overlays.retain(|id| !closing(id));
        for (id, dismissed) in
            std::iter::once((overlay, dismissed)).chain(nested.iter().map(|&id| (id, true)))
        {
            if self.contains(id) {
                self.arena.get_mut_as(id).open = false;
                self.arena.invalidate_node(id);
            }
            if dismissed {
                self.call_overlay_dismiss(id);
            }
        }
    }

    pub fn dismiss_topmost_overlay(&mut self) {
        if let Some(&top) = self.overlay_stack.last() {
            self.end_overlay(top, true);
        }
    }

    fn overlays_within(
        &self,
        overlay: NodeOf<OverlayNode>,
        candidates: &[NodeOf<OverlayNode>],
    ) -> Vec<NodeOf<OverlayNode>> {
        if candidates.is_empty() || !self.contains(overlay) {
            return Vec::new();
        }
        let mut found = Vec::new();
        let mut pending = self.arena.get(overlay).children();
        while let Some(node) = pending.pop() {
            if !self.contains(node) {
                continue;
            }
            if let Some(&candidate) = candidates.iter().find(|candidate| candidate.id() == node) {
                found.push(candidate);
                if found.len() == candidates.len() {
                    break;
                }
            }
            pending.extend(self.arena.get(node).children());
        }
        found
    }

    fn call_overlay_dismiss(&mut self, id: NodeOf<OverlayNode>) {
        if !self.contains(id) {
            return;
        }
        let mut element = self.arena.take(id);
        let handler = element
            .as_any_mut()
            .downcast_mut::<OverlayNode>()
            .and_then(|node| node.on_dismiss.take());
        self.arena.put_back(id, element);
        if let Some(mut handler) = handler {
            handler();
            if self.contains(id) {
                let node = self.arena.get_mut_as::<OverlayNode>(id);
                if node.on_dismiss.is_none() {
                    node.on_dismiss = Some(handler);
                }
            }
        }
    }

    pub fn set_overlay_light(&mut self, overlay: NodeOf<OverlayNode>, light: bool) {
        if self.arena.get_as::<OverlayNode>(overlay).light != light {
            self.arena.touch_mut_as::<OverlayNode>(overlay).light = light;
        }
    }

    pub fn set_overlay_trigger(&mut self, overlay: NodeOf<OverlayNode>, trigger: Option<NodeRef>) {
        self.arena.get_mut_as::<OverlayNode>(overlay).trigger = trigger;
    }

    fn on_overlay_trigger(&self, overlay: NodeOf<OverlayNode>, pos: Pos2) -> bool {
        self.arena
            .get_as::<OverlayNode>(overlay)
            .trigger
            .as_ref()
            .and_then(NodeRef::try_get)
            .and_then(|trigger| self.node_rect(trigger))
            .is_some_and(|rect| rect.contains_half_open(pos))
    }

    fn overlay_holds(&self, overlay: NodeOf<OverlayNode>, pos: Pos2) -> bool {
        self.overlay_content(overlay)
            .and_then(|content| self.node_rect(content))
            .is_some_and(|rect| rect.contains_half_open(pos))
    }

    pub(crate) fn light_overlay_misses(&self, overlay: NodeOf<OverlayNode>, pos: Pos2) -> bool {
        self.arena.get_as::<OverlayNode>(overlay).light && !self.overlay_holds(overlay, pos)
    }

    pub fn dismiss_light_overlays(&mut self, pos: Pos2) {
        while let Some(&top) = self.overlay_stack.last() {
            if !self.light_overlay_misses(top, pos)
                || self.on_overlay_trigger(top, pos)
                || self.arena.get_as::<OverlayNode>(top).locks
            {
                return;
            }
            self.end_overlay(top, true);
        }
    }

    pub fn pointer_passes_under_overlays(&self, pos: Pos2) -> bool {
        !self.overlay_stack.is_empty()
            && self
                .overlay_stack
                .iter()
                .all(|overlay| self.light_overlay_misses(*overlay, pos))
    }

    fn dismiss_overlay_if_outside(&mut self, overlay: NodeOf<OverlayNode>, pos: Pos2) {
        let Some(level) = self.overlay_stack.iter().position(|&id| id == overlay) else {
            return;
        };
        let inside_any = self.overlay_stack[level..].iter().any(|&id| {
            self.overlay_content(id)
                .and_then(|content| self.node_rect(content))
                .is_some_and(|rect| rect.contains_half_open(pos))
        });
        if !inside_any {
            let scrim = self.arena.get_as::<OverlayNode>(overlay).scrim;
            self.capture_pointer(scrim);
            self.forward.swallow_press();
            self.end_overlay(overlay, true);
        }
    }
}
