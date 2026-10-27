use std::rc::Rc;

use crate::painter::{Draw, Painter};

pub fn draw_gpu(drawing: Option<beui_core::drawing::Drawing>) -> Draw {
    Rc::new(move |painter, rect| {
        if let Some(drawing) = drawing.as_ref() {
            painter.core().drawing(rect, drawing);
        }
    })
}

pub fn core_painter(painter: &Painter) -> &beui_core::painter::Painter {
    painter.core()
}

pub fn build(f: impl FnOnce() -> crate::primitives::Root) -> beui_core::document::Document {
    let mut document = beui_core::document::Document::new();
    let root = beui_core::current::enter(&mut document, f);
    document.set_root(root.child());
    document
}

pub fn wrap_painter(painter: &beui_core::painter::Painter) -> Painter {
    Painter::of(painter)
}
