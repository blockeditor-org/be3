use super::*;

mod a_selected_light_is_ringed_where_it_sits;
mod a_surface_is_drawn_along_the_line_it_spans;

use block_editor_beui::beui::{RawInput, Shape};

fn shapes_of(draw: Draw, rect: Rect) -> Vec<Shape> {
    let context = block_editor_beui::beui::context();
    let output = context.run(RawInput::default(), |context| {
        draw(&context.painter(), rect);
    });
    output.shapes().to_vec()
}
