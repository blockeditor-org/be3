use std::any::Any;

use crate::base::overlay::OverlayNode;
use crate::callback::{Callback, ClickCallback};
use crate::document::Document;
use crate::geometry::{Rect, Vec2};
use crate::input::BackGesture;
use crate::node::{Element, InteractInput, NodeId, NodeOf, Rects};
use crate::painter::Painter;

pub struct BackNode {
    child: Option<NodeId>,
    enabled: bool,
    on_back: ClickCallback,
    on_gesture: Callback<BackGesture>,
}

impl Element for BackNode {
    fn measure(&self, doc: &mut Document, painter: &Painter, available: Vec2) -> Vec2 {
        match self.child {
            Some(child) => crate::layout::measure(doc, painter, child, available),
            None => Vec2::ZERO,
        }
    }

    fn baseline(&self, doc: &mut Document, painter: &Painter, available: Vec2) -> Option<f32> {
        crate::layout::baseline(doc, painter, self.child?, available)
    }

    fn layout(&mut self, doc: &mut Document, painter: &Painter, rect: Rect, out: &Rects) {
        if let Some(child) = self.child {
            crate::layout::layout(doc, painter, child, rect, out);
        }
    }

    fn paint(&self, doc: &Document, painter: &Painter, rects: &Rects, _rect: Rect) {
        if let Some(child) = self.child {
            crate::paint::paint(doc, painter, rects, child);
        }
    }

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
        children.extend(self.child);
    }

    fn children(&self) -> Vec<NodeId> {
        self.child.into_iter().collect()
    }

    fn kind(&self) -> &'static str {
        "back handler"
    }

    fn detail(&self) -> Option<String> {
        Some(if self.enabled { "enabled" } else { "disabled" }.to_owned())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Document {
    pub fn create_back_handler(
        &mut self,
        child: NodeId,
        on_back: ClickCallback,
        on_gesture: Callback<BackGesture>,
    ) -> NodeOf<BackNode> {
        let id = self.arena.insert(BackNode {
            child: Some(child),
            enabled: true,
            on_back,
            on_gesture,
        });
        self.back_handlers.push(id);
        id
    }

    pub fn set_back_handler_enabled(&mut self, handler: NodeOf<BackNode>, enabled: bool) {
        if self.arena.get_as::<BackNode>(handler).enabled == enabled {
            return;
        }
        self.arena.get_mut_as::<BackNode>(handler).enabled = enabled;
        if !enabled && self.back_gesture == Some(handler.id()) {
            self.back_gesture = None;
        }
    }

    pub fn handles_back(&self) -> bool {
        self.back_target().is_some()
    }

    fn back_target(&self) -> Option<NodeId> {
        let modal = self.overlay_stack.last().copied();
        let handler = self
            .back_handlers
            .iter()
            .rev()
            .copied()
            .find(|handler| {
                self.contains(handler.id())
                    && self.arena.get_as(*handler).enabled
                    && self.owning_modal(handler.id()) == Some(modal)
            })
            .map(NodeOf::id);
        handler.or(modal.map(NodeOf::id))
    }

    fn owning_modal(&self, node: NodeId) -> Option<Option<NodeOf<OverlayNode>>> {
        let mut current = node;
        loop {
            if let Some(overlay) = self.arena.kind_of::<OverlayNode>(current) {
                if !self.is_overlay_open(current) {
                    return None;
                }
                if self.overlay_stack.contains(&overlay) {
                    return Some(Some(overlay));
                }
            }
            match self.arena.parent(current) {
                Some(parent) => current = parent,
                None => return (self.root == Some(current)).then_some(None),
            }
        }
    }

    pub fn back(&mut self, gesture: BackGesture) {
        let target = match (gesture, self.back_gesture) {
            (BackGesture::Started { .. }, _) | (_, None) => self.back_target(),
            (_, Some(target)) => Some(target),
        };
        self.back_gesture = match gesture {
            BackGesture::Started { .. } => target,
            BackGesture::Progressed(_) => self.back_gesture,
            BackGesture::Cancelled | BackGesture::Invoked => None,
        };
        let Some(target) = target.filter(|target| self.contains(*target)) else {
            return;
        };
        if let Some(on_gesture) = self.gesture_taker(target) {
            on_gesture.call(match gesture {
                BackGesture::Progressed(progress) => {
                    BackGesture::Progressed(progress.clamp(0.0, 1.0))
                }
                gesture => gesture,
            });
            return;
        }
        if gesture == BackGesture::Invoked {
            self.invoke_back(target);
        }
    }

    fn gesture_taker(&self, target: NodeId) -> Option<Callback<BackGesture>> {
        let handler = self.arena.kind_of::<BackNode>(target)?;
        let on_gesture = &self.arena.get_as(handler).on_gesture;
        (!on_gesture.is_empty()).then(|| on_gesture.clone())
    }

    fn invoke_back(&mut self, target: NodeId) {
        if let Some(overlay) = self.arena.kind_of::<OverlayNode>(target) {
            self.close_overlay(overlay);
            return;
        }
        let Some(handler) = self.arena.kind_of::<BackNode>(target) else {
            return;
        };
        let on_back = self.arena.get_as(handler).on_back.clone();
        on_back.call();
    }
}
