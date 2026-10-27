use super::*;

mod a_selected_light_is_ringed_where_it_sits;
mod a_surface_is_drawn_along_the_line_it_spans;

use block_editor_beui::beui::native::Shape;

fn shapes_of(draw: Draw, rect: Rect) -> Vec<Shape> {
    let context = block_editor_beui::beui::native::context();
    let output = context.run(|context| {
        draw(
            &block_editor_beui::beui::native::wrap_painter(&context.painter()),
            rect,
        );
    });
    output.main().shapes().to_vec()
}
