use std::any::Any;

use crate::base::overlay::OverlayNode;
use crate::callback::ClickCallback;
use crate::document::Document;
use crate::geometry::{Rect, Vec2, vec2};
use crate::input::{BackEdge, BackGesture};
use crate::node::{Element, InteractInput, NodeId, NodeOf, Rects};
use crate::painter::Painter;

const SHIFT_FRACTION: f32 = 0.1;
const MAX_SHIFT: f32 = 64.0;

#[derive(Clone, Copy, Default, PartialEq, Debug)]
pub struct BackProgress {
    pub progress: f32,
    pub edge: BackEdge,
}

impl BackProgress {
    pub fn shift(self, width: f32) -> Vec2 {
        let distance = self.progress * (width * SHIFT_FRACTION).min(MAX_SHIFT);
        match self.edge {
            BackEdge::Right => vec2(-distance, 0.0),
            BackEdge::Left | BackEdge::None => vec2(distance, 0.0),
        }
    }
}

pub struct BackNode {
    child: Option<NodeId>,
    enabled: bool,
    progress: BackProgress,
    on_back: ClickCallback,
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
            let rect = rect.translate(self.progress.shift(rect.width()));
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
    ) -> NodeOf<BackNode> {
        let id = self.arena.insert(BackNode {
            child: Some(child),
            enabled: true,
            progress: BackProgress::default(),
            on_back,
        });
        self.back_handlers.push(id);
        id
    }

    pub fn set_back_handler_enabled(&mut self, handler: NodeOf<BackNode>, enabled: bool) {
        if self.arena.get_as::<BackNode>(handler).enabled == enabled {
            return;
        }
        self.arena.get_mut_as::<BackNode>(handler).enabled = enabled;
        if !enabled {
            self.set_back_progress(handler.id(), BackProgress::default());
        }
    }

    pub fn handles_back(&self) -> bool {
        self.back_target().is_some()
    }

    fn back_target(&self) -> Option<NodeId> {
        if let Some(overlay) = self.overlay_stack.last() {
            return Some(overlay.id());
        }
        self.back_handlers
            .iter()
            .rev()
            .copied()
            .find(|handler| self.contains(handler.id()) && self.arena.get_as(*handler).enabled)
            .map(NodeOf::id)
    }

    pub fn back(&mut self, gesture: BackGesture) {
        match gesture {
            BackGesture::Started { edge } => {
                self.end_back_gesture();
                if let Some(target) = self.back_target() {
                    self.back_gesture = Some((target, edge));
                }
            }
            BackGesture::Progressed(progress) => {
                if let Some((target, edge)) = self.back_gesture {
                    let progress = progress.clamp(0.0, 1.0);
                    self.set_back_progress(target, BackProgress { progress, edge });
                }
            }
            BackGesture::Cancelled => self.end_back_gesture(),
            BackGesture::Invoked => {
                let target = match self.back_gesture {
                    Some((target, _)) => Some(target),
                    None => self.back_target(),
                };
                self.end_back_gesture();
                if let Some(target) = target {
                    self.invoke_back(target);
                }
            }
        }
    }

    fn end_back_gesture(&mut self) {
        if let Some((target, _)) = self.back_gesture.take() {
            self.set_back_progress(target, BackProgress::default());
        }
    }

    fn set_back_progress(&mut self, target: NodeId, progress: BackProgress) {
        if !self.contains(target) {
            return;
        }
        if let Some(overlay) = self.arena.kind_of::<OverlayNode>(target) {
            self.set_overlay_back_progress(overlay, progress);
            return;
        }
        let Some(handler) = self.arena.kind_of::<BackNode>(target) else {
            return;
        };
        if self.arena.get_as(handler).progress != progress {
            self.arena.get_mut_as(handler).progress = progress;
            self.arena.invalidate_node(target);
        }
    }

    fn invoke_back(&mut self, target: NodeId) {
        if !self.contains(target) {
            return;
        }
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
