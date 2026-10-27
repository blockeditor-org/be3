use super::*;
use beui_types::text::FontId;
use beui_types::Color32;

fn glyph(range: Range<usize>, glyph: &str) -> TextGlyph {
    TextGlyph::new(
        range,
        glyph,
        FontId::monospace(12.0),
        Color32::from_rgb(200, 0, 0),
    )
}

fn plain(range: Range<usize>) -> Segment {
    Segment::Plain(range)
}

fn shown(range: Range<usize>, glyph: &str) -> Segment {
    Segment::Glyph {
        range,
        glyph: glyph.to_owned(),
        font: FontId::monospace(12.0),
        color: Color32::from_rgb(200, 0, 0).into(),
    }
}

fn stops_of(pieces: &[(u32, &str)]) -> Vec<Stop<u32>> {
    let mut stops = Vec::new();
    let mut at = 0;
    for (node, text) in pieces {
        text_stops(at, text, node, &mut stops);
        at += text.len();
    }
    stops
}

mod a_glyph_belongs_to_the_run_it_starts_in_and_is_cut_at_that_runs_end;
mod a_glyph_ending_inside_a_character_stops_before_it;
mod a_run_without_glyphs_is_one_plain_segment_even_when_empty;
mod atoms_take_their_bytes_and_breaks_count_as_new_lines;
mod glyphs_cut_a_run_at_the_bytes_they_cover_counted_from_the_text_start;
mod glyphs_that_overlap_or_cover_nothing_are_left_out;
mod inline_items_stand_for_their_text_or_an_object_replacement_character;
mod the_characters_around_a_byte_are_found_across_text_nodes;
mod utf16_offsets_in_a_text_node_map_to_the_utf8_bytes_of_the_whole_text;
