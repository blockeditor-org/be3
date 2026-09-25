use std::any::Any;
use std::cell::Cell;
use std::rc::Rc;

use crate::color::Color32;
use crate::geometry::{Pos2, Rect, Vec2, pos2};
use crate::input::{CursorIcon, PointerPress};
use crate::painter::Painter;

use beui_macros::{component, view};

use crate::document::Document;
use crate::node::{ClickHandler, Element, InteractInput, NodeId, NodeMap};
use crate::reactive::{
    Child, ClickCallback, ClickCatcher, IntoProp, NodeRef, Prop, create_effect, with_document,
    with_reactive_scope,
};

#[derive(Clone, PartialEq)]
pub(crate) enum OverlayAnchor {
    Node(NodeRef),
    Point(Pos2),
}

impl Default for OverlayAnchor {
    fn default() -> Self {
        OverlayAnchor::Point(Pos2::ZERO)
    }
}

impl IntoProp<OverlayAnchor> for &NodeRef {
    fn into_prop(self) -> Prop<OverlayAnchor> {
        Prop::Static(OverlayAnchor::Node(self.clone()))
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Placement {
    At,
    BelowStart,
    RightStart,
    Center,
    InsideTop,
    InsideBottom,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
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

pub(crate) struct OverlayNode {
    content: Option<NodeId>,
    scrim: NodeId,
    dim: Color32,
    open: bool,
    anchor: OverlayAnchor,
    placement: Placement,
    traps_focus: bool,
    mode: OverlayMode,
    on_dismiss: Option<ClickHandler>,
}

impl OverlayNode {
    fn new(scrim: NodeId, anchor: OverlayAnchor, placement: Placement) -> Self {
        Self {
            content: None,
            scrim,
            dim: Color32::TRANSPARENT,
            open: false,
            anchor,
            placement,
            traps_focus: true,
            mode: OverlayMode::Modal,
            on_dismiss: None,
        }
    }

    pub(crate) fn is_open(&self) -> bool {
        self.open
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
    let mut origin = match placement {
        Placement::BelowStart => pos2(anchor_rect.left(), anchor_rect.bottom()),
        Placement::RightStart => pos2(anchor_rect.right(), anchor_rect.top()),
        Placement::At | Placement::Center | Placement::InsideTop | Placement::InsideBottom => {
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

    fn layout(
        &mut self,
        doc: &mut Document,
        painter: &Painter,
        _rect: Rect,
        out: &mut NodeMap<Rect>,
    ) {
        if !self.open {
            return;
        }
        let Some(content) = self.content else {
            return;
        };
        let painter = &painter.ctx().painter();
        let viewport = doc.viewport_rect();
        crate::layout::layout(doc, painter, self.scrim, viewport, out);
        let content_size = crate::layout::measure(doc, painter, content, viewport.size());
        let anchor_rect = match &self.anchor {
            OverlayAnchor::Node(node) => node
                .try_get()
                .and_then(|id| out.get(&id).copied())
                .unwrap_or(viewport),
            OverlayAnchor::Point(pos) => Rect::from_min_size(*pos, Vec2::ZERO),
        };
        let rect = resolve_rect(viewport, anchor_rect, self.placement, content_size);
        crate::layout::layout(doc, painter, content, rect, out);
    }

    fn paint(&self, doc: &Document, painter: &Painter, _rects: &NodeMap<Rect>, _rect: Rect) {
        if self.paints() {
            painter.rect_filled(doc.viewport_rect(), 0.0, self.dim);
        }
    }

    fn paints(&self) -> bool {
        self.open && self.dim.alpha() > 0
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
        children.push(self.scrim);
        children.extend(self.content);
    }

    fn children(&self) -> Vec<NodeId> {
        let mut children = vec![self.scrim];
        children.extend(self.content);
        children
    }

    fn kind(&self) -> &'static str {
        "overlay"
    }

    fn detail(&self) -> Option<String> {
        Some(if self.open { "open" } else { "closed" }.to_owned())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

#[component]
pub(crate) fn Overlay(
    anchor: Prop<OverlayAnchor>,
    #[prop(default = Placement::BelowStart)] placement: Prop<Placement>,
    #[prop(default = Color32::TRANSPARENT)] scrim: Prop<Color32>,
    #[prop(default = true)] traps_focus: Prop<bool>,
    #[prop(default = OverlayMode::Modal)] mode: Prop<OverlayMode>,
    open: Prop<bool>,
    on_dismiss: ClickCallback,
    children: Option<Child>,
) -> NodeId {
    let content = children;
    let overlay = with_document(|document| {
        let overlay = document.create_overlay(anchor.peek(), Placement::BelowStart);
        if let Some(content) = content {
            document.set_overlay_content(overlay, content);
        }
        if !on_dismiss.is_empty() {
            document.set_overlay_on_dismiss(overlay, move || on_dismiss.call());
        }
        overlay
    });
    create_effect(move || {
        with_document(|document| document.set_overlay_anchor(overlay, anchor.get()))
    });
    create_effect(move || {
        with_document(|document| document.set_overlay_placement(overlay, placement.get()))
    });
    create_effect(move || {
        with_document(|document| document.set_overlay_scrim(overlay, scrim.get()))
    });
    create_effect(move || {
        with_document(|document| document.set_overlay_traps_focus(overlay, traps_focus.get()))
    });
    create_effect(move || with_document(|document| document.set_overlay_mode(overlay, mode.get())));
    create_effect(move || {
        let open = open.get();
        with_document(|document| match open {
            true => document.open_overlay(overlay),
            false => document.close_overlay(overlay),
        });
    });
    overlay
}

impl Document {
    pub(crate) fn create_overlay(&mut self, anchor: OverlayAnchor, placement: Placement) -> NodeId {
        let overlay_cell: Rc<Cell<Option<NodeId>>> = Rc::new(Cell::new(None));
        let press_cell = overlay_cell.clone();
        let scrim = with_reactive_scope(self, || {
            view! {
                <ClickCatcher
                    cursor=CursorIcon::Default
                    on_press={move |press: PointerPress| {
                        let id = press_cell.get().expect("overlay not yet initialized");
                        with_document(|document| document.dismiss_overlay_if_outside(id, press.pos));
                    }}
                ></ClickCatcher>
            }
        });
        let id = self
            .arena
            .insert(OverlayNode::new(scrim, anchor, placement));
        overlay_cell.set(Some(id));
        id
    }

    pub(crate) fn set_overlay_content(&mut self, overlay: NodeId, content: NodeId) {
        self.arena.get_mut_as::<OverlayNode>(overlay).content = Some(content);
    }

    pub(crate) fn set_overlay_anchor(&mut self, overlay: NodeId, anchor: OverlayAnchor) {
        self.arena.get_mut_as::<OverlayNode>(overlay).anchor = anchor;
    }

    pub(crate) fn set_overlay_scrim(&mut self, overlay: NodeId, color: Color32) {
        if self.arena.get_as::<OverlayNode>(overlay).dim != color {
            self.arena.get_mut_as::<OverlayNode>(overlay).dim = color;
        }
    }

    pub(crate) fn set_overlay_placement(&mut self, overlay: NodeId, placement: Placement) {
        if self.arena.get_as::<OverlayNode>(overlay).placement != placement {
            self.arena.get_mut_as::<OverlayNode>(overlay).placement = placement;
        }
    }

    pub(crate) fn set_overlay_traps_focus(&mut self, overlay: NodeId, traps_focus: bool) {
        if self.arena.get_as::<OverlayNode>(overlay).traps_focus != traps_focus {
            self.arena.get_mut_as::<OverlayNode>(overlay).traps_focus = traps_focus;
        }
    }

    pub(crate) fn set_overlay_mode(&mut self, overlay: NodeId, mode: OverlayMode) {
        if self.arena.get_as::<OverlayNode>(overlay).mode == mode {
            return;
        }
        self.arena.get_mut_as::<OverlayNode>(overlay).mode = mode;
        if self.arena.get_as::<OverlayNode>(overlay).open {
            self.close_overlay(overlay);
            self.arena.get_mut_as::<OverlayNode>(overlay).open = false;
            self.open_overlay(overlay);
        }
    }

    pub(crate) fn overlay_is_floating(&self, overlay: NodeId) -> bool {
        self.contains(overlay)
            && self.arena.get_as::<OverlayNode>(overlay).mode == OverlayMode::Floating
            && self.arena.get_as::<OverlayNode>(overlay).open
    }

    pub(crate) fn floating_overlays(&self) -> Vec<NodeId> {
        self.passive_overlays
            .iter()
            .copied()
            .filter(|overlay| self.overlay_is_floating(*overlay))
            .collect()
    }

    pub(crate) fn overlays_bottom_up(&self) -> Vec<NodeId> {
        let floating = self.floating_overlays();
        let passive = self
            .passive_overlays
            .iter()
            .filter(|overlay| !floating.contains(overlay));
        floating
            .iter()
            .chain(self.overlay_stack.iter())
            .chain(passive)
            .copied()
            .collect()
    }

    pub(crate) fn pointer_layers(&self, root: NodeId) -> Vec<NodeId> {
        match self.overlay_stack.is_empty() {
            true => self
                .floating_overlays()
                .into_iter()
                .rev()
                .chain([root])
                .collect(),
            false => self.overlay_stack.iter().rev().copied().collect(),
        }
    }

    pub(crate) fn floating_covers(&self, pos: Pos2) -> bool {
        if self.modal_open() {
            return false;
        }
        self.floating_overlays().into_iter().any(|overlay| {
            self.overlay_content(overlay)
                .and_then(|content| self.node_rect(content))
                .is_some_and(|rect| rect.contains(pos))
        })
    }

    pub(crate) fn raise_overlay(&mut self, overlay: NodeId) {
        let Some(index) = self.passive_overlays.iter().position(|id| *id == overlay) else {
            return;
        };
        if index + 1 == self.passive_overlays.len() {
            return;
        }
        let raised = self.passive_overlays.remove(index);
        self.passive_overlays.push(raised);
        self.arena.invalidate_node(raised);
    }

    pub(crate) fn overlay_traps_focus(&self, overlay: NodeId) -> bool {
        self.arena.get_as::<OverlayNode>(overlay).traps_focus
    }

    pub(crate) fn set_overlay_on_dismiss(
        &mut self,
        overlay: NodeId,
        handler: impl FnMut() + 'static,
    ) {
        self.arena.get_mut_as::<OverlayNode>(overlay).on_dismiss = Some(Box::new(handler));
    }

    #[cfg(test)]
    pub(crate) fn is_overlay_open(&self, overlay: NodeId) -> bool {
        self.arena.get_as::<OverlayNode>(overlay).open
    }

    pub(crate) fn overlay_content(&self, overlay: NodeId) -> Option<NodeId> {
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
            .filter_map(|overlay| self.overlay_content(*overlay))
            .filter_map(|content| self.node_rect(content))
            .filter(|rect| rect.is_positive())
            .collect()
    }

    pub(crate) fn open_overlay(&mut self, overlay: NodeId) {
        if self.arena.get_as::<OverlayNode>(overlay).open {
            return;
        }
        self.arena.get_mut_as::<OverlayNode>(overlay).open = true;
        match self.arena.get_as::<OverlayNode>(overlay).mode.stacked() {
            true => self.overlay_stack.push(overlay),
            false => self.passive_overlays.push(overlay),
        }
        self.arena.invalidate_node(overlay);
    }

    pub(crate) fn close_overlay(&mut self, overlay: NodeId) {
        if let Some(level) = self.overlay_stack.iter().position(|&id| id == overlay) {
            self.close_overlay_at(level);
            return;
        }
        if let Some(index) = self.passive_overlays.iter().position(|&id| id == overlay) {
            self.passive_overlays.remove(index);
            if self.contains(overlay) {
                self.arena.get_mut_as::<OverlayNode>(overlay).open = false;
                self.arena.invalidate_node(overlay);
            }
            self.call_overlay_dismiss(overlay);
        }
    }

    pub(crate) fn close_topmost_overlay(&mut self) {
        if !self.overlay_stack.is_empty() {
            self.close_overlay_at(self.overlay_stack.len() - 1);
        }
    }

    fn close_overlay_at(&mut self, level: usize) {
        let closing: Vec<NodeId> = self.overlay_stack.split_off(level);
        for id in closing {
            if self.contains(id) {
                self.arena.get_mut_as::<OverlayNode>(id).open = false;
            }
            self.arena.invalidate_node(id);
            self.call_overlay_dismiss(id);
        }
    }

    fn call_overlay_dismiss(&mut self, id: NodeId) {
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

    fn dismiss_overlay_if_outside(&mut self, overlay: NodeId, pos: Pos2) {
        let Some(level) = self.overlay_stack.iter().position(|&id| id == overlay) else {
            return;
        };
        let inside_any = self.overlay_stack[level..].iter().any(|&id| {
            self.overlay_content(id)
                .and_then(|content| self.node_rect(content))
                .is_some_and(|rect| rect.contains(pos))
        });
        if !inside_any {
            let scrim = self.arena.get_as::<OverlayNode>(overlay).scrim;
            self.capture_pointer(scrim);
            self.close_overlay_at(level);
        }
    }
}
