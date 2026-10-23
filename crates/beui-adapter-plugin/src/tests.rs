use std::cell::Cell;

use beui_core::geometry::{pos2, vec2};
use beui_core::input::Event;
use block_editor_plugin::{EditorRegion, FrameSpec, Input};

use super::*;

mod a_copy_in_a_region_reaches_the_host;
mod a_pointer_lands_where_the_region_is_scrolled_to;
mod a_region_describes_itself_and_does_what_a_screen_reader_asks;
mod a_region_lays_out_over_its_whole_rect_not_just_what_shows;
mod a_region_tells_the_host_it_locks_the_pointer_and_when_it_stops;
mod a_region_that_loses_the_keyboard_mid_press_keeps_the_press;
mod a_regions_keyboard_reaches_the_host_with_its_asks;
mod a_trackpad_scroll_held_past_the_start_stretches_and_springs_back_when_lifted;
mod an_image_copied_in_a_region_reaches_the_host;

fn region(rect: Rect, pixels: [u32; 2]) -> Region {
    Region {
        region: EditorRegion::Frame,
        rect,
        scale_factor: 1.0,
        pixels,
        age: 0,
        spec: FrameSpec::default(),
        monitors: Vec::new(),
        describe: None,
        seen: true,
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
