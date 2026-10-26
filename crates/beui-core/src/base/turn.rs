use std::any::Any;
use std::f32::consts::FRAC_PI_2;

use crate::document::Document;
use crate::geometry::{Rect, Vec2, vec2};
use crate::node::{Element, NodeId, NodeOf, Rects};
use crate::painter::Painter;

pub struct TurnNode {
    child: Option<NodeId>,
    quarters: u8,
}

impl TurnNode {
    pub fn quarter_turns(&self) -> u8 {
        self.quarters
    }

    fn sideways(&self) -> bool {
        self.quarters % 2 == 1
    }

    fn swapped(&self, size: Vec2) -> Vec2 {
        match self.sideways() {
            true => vec2(size.y, size.x),
            false => size,
        }
    }
}

pub fn turn_angle(quarters: u8) -> f32 {
    f32::from(quarters % 4) * FRAC_PI_2
}

impl Element for TurnNode {
    fn paints_from_layout(&self) -> bool {
        false
    }

    fn turns_children(&self) -> bool {
        true
    }

    fn measure(&self, doc: &mut Document, painter: &Painter, available: Vec2) -> Vec2 {
        self.child.map_or(Vec2::ZERO, |child| {
            let measured = crate::layout::measure(doc, painter, child, self.swapped(available));
            self.swapped(measured)
        })
    }

    fn layout(&mut self, doc: &mut Document, painter: &Painter, rect: Rect, out: &Rects) {
        let Some(child) = self.child else {
            return;
        };
        let turned = Rect::from_center_size(rect.center(), self.swapped(rect.size()));
        crate::layout::layout(doc, painter, child, turned, out);
    }

    fn paint(&self, doc: &Document, painter: &Painter, rects: &Rects, rect: Rect) {
        let Some(child) = self.child else {
            return;
        };
        let turned = painter.rotated(rect.center(), turn_angle(self.quarters));
        crate::paint::paint(doc, &turned, rects, child);
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
        "turn"
    }

    fn detail(&self) -> Option<String> {
        let quarters = self.quarters;
        (quarters != 0).then(|| format!("{quarters} quarter turns"))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Document {
    pub fn create_turn(&mut self, child: NodeId) -> NodeOf<TurnNode> {
        self.arena.insert(TurnNode {
            child: Some(child),
            quarters: 0,
        })
    }

    pub fn set_turn(&mut self, turn: NodeOf<TurnNode>, quarters: u8) {
        let quarters = quarters % 4;
        if self.arena.get_as::<TurnNode>(turn).quarters != quarters {
            self.arena.get_mut_as::<TurnNode>(turn).quarters = quarters;
        }
    }
}
