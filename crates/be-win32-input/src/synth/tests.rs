use super::flags::*;
use super::*;
use beui_core::geometry::{pos2, vec2};
use beui_core::input::Key;

const DESKTOP: PixelRect = PixelRect::new(0, 0, 2000, 1000);

fn moved_to(synth: Synth) -> (i32, i32, u32) {
    match synth {
        Synth::Mouse { x, y, flags, .. } => (x, y, flags),
        Synth::Key { .. } => panic!("a key, not the mouse"),
    }
}

mod a_key_is_injected_by_its_scan_code;
mod a_keystroke_prefers_its_physical_key_and_falls_back_to_its_text;
mod a_move_goes_to_the_point_in_virtual_desktop_coordinates;
mod a_press_and_release_happen_where_the_pointer_is;
mod a_scroll_turns_the_wheel_a_notch_a_line_the_way_winit_reads_it;
mod an_event_in_an_entry_is_planned_at_its_place_on_the_frame;
mod text_types_each_utf16_unit_and_presses_enter_and_tab;
