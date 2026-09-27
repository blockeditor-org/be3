use super::*;
use crate::color::Color32;

mod appending_paint_keeps_shared_frames_intact;
mod composing_retained_documents_preserves_paint_order;
mod equal_painting_adopts_the_latest_retained_buffer;
mod transforming_paint_keeps_the_retained_source_intact;

fn retained() -> Rc<Vec<Shape>> {
    let context = Context::new();
    context
        .run(RawInput::default(), |ctx| {
            ctx.painter().rect_filled(rect(), 0.0, Color32::WHITE);
        })
        .shapes
}

fn rect() -> Rect {
    Rect::from_min_max(pos2(10.0, 10.0), pos2(30.0, 30.0))
}
