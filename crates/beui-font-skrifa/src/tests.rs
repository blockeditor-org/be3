use super::*;
use beui_core::font::Fonts;
use beui_core::rich::BREAK_ZONE_EMS;
use beui_types::{FontId, TextLayout};

mod a_character_no_font_covers_is_looked_up_once_and_drawn_with_what_is_found;
mod a_colour_bitmap_glyph_is_scaled_to_the_text_in_its_own_colours;
mod a_full_galley_cache_keeps_what_is_still_being_used;
mod a_galley_is_reused_at_every_width_that_breaks_it_the_same_way;
mod a_good_break_inside_the_zone_is_taken;
mod a_long_word_after_a_short_one_breaks_at_the_edge_not_back_at_the_space;
mod a_path_breaks_after_its_punctuation_when_no_space_is_near;
mod a_subpixel_phase_never_leaves_its_whole_pixel;
mod a_word_that_fits_stays_on_its_line;
mod a_word_too_long_for_any_line_never_widens_the_text;
mod a_word_too_long_for_the_first_line_breaks_where_its_letters_stop_fitting;
mod a_word_wider_than_the_wrap_width_breaks_between_its_letters;
mod a_wrapped_text_laid_out_at_its_own_width_keeps_its_lines;
mod an_edit_on_the_first_line_leaves_the_lines_below_drawn_as_they_were;
mod emboldening_grows_a_square_by_exactly_the_strength;
mod emboldening_keeps_a_sharp_spike_bounded;
mod emboldening_shrinks_a_counter;
mod every_glyph_draws_inside_the_box_it_is_culled_by;
mod fonts_added_later_redraw_the_text_that_was_missing_them;
mod layered_colour_glyphs_are_drawn_in_their_palette_colours;
mod letters_land_where_their_advances_put_them;
mod replacing_the_fonts_lays_the_text_out_again;
mod spaces_after_the_last_word_that_fits_hang_past_the_edge;

const SIZE: f32 = 14.0;

fn font() -> FontId {
    FontId::proportional(SIZE)
}

fn fonts() -> Fonts {
    Fonts::new(SkrifaFonts::default())
}

fn zone() -> f32 {
    SIZE * BREAK_ZONE_EMS
}

fn width_of(fonts: &mut Fonts, text: &str) -> f32 {
    fonts
        .layout(text, font(), TextLayout::DEFAULT, 1.0)
        .cursor_pos(beui_types::pos2(0.0, 0.0), text.len())
        .x
}

fn lines(fonts: &mut Fonts, text: &str, width: f32) -> Vec<String> {
    let galley = fonts.layout(text, font(), TextLayout::wrapped(width), 1.0);
    galley
        .lines()
        .iter()
        .map(|line| galley.text()[line.range.clone()].to_owned())
        .collect()
}
