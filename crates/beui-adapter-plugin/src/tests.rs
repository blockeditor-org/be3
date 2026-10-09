use std::cell::Cell;

use beui_core::geometry::{pos2, vec2};
use beui_core::input::Event;
use block_editor_plugin::{EditorRegion, FrameSpec};

use super::*;

mod a_copy_in_a_region_reaches_the_host;
mod a_key_carries_the_modifiers_held_before_it;
mod a_pointer_lands_where_the_region_is_scrolled_to;
mod a_region_lays_out_over_its_whole_rect_not_just_what_shows;
mod losing_focus_ends_an_emulated_touch;

fn region(rect: Rect, pixels: [u32; 2]) -> Region {
    Region {
        region: EditorRegion::Frame,
        rect,
        scale_factor: 1.0,
        pixels,
        age: 0,
        spec: FrameSpec::default(),
        monitors: Vec::new(),
    }
}

struct Recording {
    laid: Rc<Cell<Option<Rect>>>,
    copy: Option<String>,
}

impl App for Recording {
    fn update(&mut self, context: &Context, rect: Rect) {
        self.laid.set(Some(rect));
        if let Some(text) = self.copy.take() {
            context.copy_text(text);
        }
    }
}
