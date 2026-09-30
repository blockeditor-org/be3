use std::rc::Rc;

use crate::base::drawing::Draw;
use crate::color::Color32;
use crate::font::Galley;
use crate::geometry::{Pos2, Rect, Vec2};
use crate::painter::Painter;

#[derive(Clone)]
pub enum PageShape {
    Rect {
        rect: Rect,
        corner_radius: f32,
        color: Color32,
    },
    Outline {
        rect: Rect,
        corner_radius: f32,
        width: f32,
        color: Color32,
    },
    Text {
        origin: Pos2,
        galley: Galley,
        color: Color32,
    },
}

impl PageShape {
    fn bounds(&self) -> Rect {
        match self {
            Self::Rect { rect, .. } | Self::Outline { rect, .. } => *rect,
            Self::Text { origin, galley, .. } => Rect::from_min_size(*origin, galley.size()),
        }
    }

    fn paint(&self, painter: &Painter, offset: Vec2) {
        match self {
            Self::Rect {
                rect,
                corner_radius,
                color,
            } => painter.rect_filled(rect.translate(offset), *corner_radius, *color),
            Self::Outline {
                rect,
                corner_radius,
                width,
                color,
            } => painter.rect_stroke(rect.translate(offset), *corner_radius, *width, *color),
            Self::Text {
                origin,
                galley,
                color,
            } => painter.galley(*origin + offset, galley.clone(), *color),
        }
    }
}

#[derive(Clone, Default)]
pub struct Page(Rc<Vec<PageShape>>);

impl Page {
    pub fn new(shapes: Vec<PageShape>) -> Self {
        Self(Rc::new(shapes))
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn draw(&self) -> Draw {
        let page = self.clone();
        Rc::new(move |painter: &Painter, rect: Rect| {
            let offset = rect.min.to_vec2();
            let clip = painter.clip_rect();
            for shape in page.0.iter() {
                if shape.bounds().translate(offset).intersects(clip) {
                    shape.paint(painter, offset);
                }
            }
        })
    }
}

impl PartialEq for Page {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}
