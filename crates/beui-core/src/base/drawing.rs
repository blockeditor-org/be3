use std::any::Any;
use std::rc::Rc;

use crate::document::Document;
use crate::geometry::{Rect, Vec2};
use crate::node::{Element, NodeId, NodeOf, Rects};
use crate::painter::Painter;

pub type Draw = Rc<dyn Fn(&Painter, Rect)>;

pub fn draw_gpu(drawing: Option<crate::drawing::Drawing>) -> Draw {
    Rc::new(move |painter, rect| {
        if let Some(drawing) = drawing.as_ref() {
            painter.drawing(rect, drawing);
        }
    })
}

pub struct DrawingNode {
    draw: Option<Draw>,
    size: Option<Vec2>,
}

impl Element for DrawingNode {
    fn paints_from_layout(&self) -> bool {
        false
    }

    fn measure(&self, _doc: &mut Document, _painter: &Painter, available: Vec2) -> Vec2 {
        let Some(size) = self.size else {
            return Vec2::ZERO;
        };
        if !available.x.is_finite() || available.x <= 0.0 || size.x <= available.x {
            return size;
        }
        Vec2::new(available.x, size.y * available.x / size.x)
    }

    fn layout(&mut self, _doc: &mut Document, _painter: &Painter, _rect: Rect, _out: &Rects) {}

    fn paint(&self, doc: &Document, painter: &Painter, _rects: &Rects, rect: Rect) {
        let Some(draw) = self.draw.as_ref() else {
            return;
        };
        crate::painter::with_colors(doc.color_definitions(), doc.color_scheme(), || {
            match self.size {
                Some(_) => draw(&painter.with_clip_rect(rect), rect),
                None => draw(painter, rect),
            }
        });
    }

    fn children(&self) -> Vec<NodeId> {
        Vec::new()
    }

    fn remove_child(&mut self, _child: NodeId) {}

    fn kind(&self) -> &'static str {
        "drawing"
    }

    fn detail(&self) -> Option<String> {
        self.size.map(|size| format!("{}x{}", size.x, size.y))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Document {
    pub fn create_drawing(&mut self) -> NodeOf<DrawingNode> {
        self.arena.insert(DrawingNode {
            draw: None,
            size: None,
        })
    }

    pub fn set_drawing(&mut self, drawing: NodeOf<DrawingNode>, draw: Draw) {
        let held = self.arena.get_as::<DrawingNode>(drawing).draw.as_ref();
        if held.is_some_and(|held| Rc::ptr_eq(held, &draw)) {
            return;
        }
        self.arena.paint_mut_as::<DrawingNode>(drawing).draw = Some(draw);
    }

    pub fn set_drawing_size(&mut self, drawing: NodeOf<DrawingNode>, size: Option<Vec2>) {
        if self.arena.get_as::<DrawingNode>(drawing).size != size {
            self.arena.get_mut_as::<DrawingNode>(drawing).size = size;
        }
    }
}
