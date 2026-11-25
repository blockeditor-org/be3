use super::*;
#[allow(unused_imports)]
use beui_paint_format::*;
#[allow(unused_imports)]
use std::collections::BTreeMap;

mod a_faded_rect_fades_toward_the_edge_it_names;
mod a_glyph_paints_its_coverage_in_its_colour;
mod a_group_paints_its_shapes_as_one_at_its_opacity;
mod a_painting_placed_with_a_corner_radius_shows_its_corners_rounded;
mod a_painting_rasterizes_onto_a_transparent_background;
mod a_rounded_rect_covers_its_corners_by_the_area_inside_them;
mod a_texture_drawn_smaller_averages_the_texels_it_covers;
mod a_transparent_texel_leaves_what_is_behind_it;
mod a_triangle_is_filled_with_its_corner_colour;
mod a_turned_rounded_rect_is_covered_where_it_turned_to;
mod tidying_a_frame_keeps_its_pixels_and_drops_what_none_of_them_show;

#[allow(unused_imports)]
use beui_paint_snapshot::{Snapshot, Texture};

#[allow(dead_code)]
fn white() -> Texture {
    Texture::encode([1, 1], &[[255, 255, 255, 255]]).unwrap()
}

#[allow(dead_code)]
fn triangle(colour: [u8; 4]) -> Snapshot {
    Snapshot::of(frame(colour), BTreeMap::from([(0, white())]))
}

#[allow(dead_code)]
fn frame(colour: [u8; 4]) -> Frame {
    let corner = |x: f32, y: f32| Vertex {
        pos: [x, y],
        uv: [0.5, 0.5],
        color: colour,
    };
    Frame {
        size: [8, 8],
        pixels_per_point: 1.0,
        background: [0, 0, 0, 255],
        primitives: vec![Primitive {
            clip: [0.0, 0.0, 8.0, 8.0],
            fade: Fade::NONE,
            rounds: Vec::new(),
            content: Content::Mesh(vec![Triangle {
                texture: 0,
                corners: [corner(0.0, 0.0), corner(8.0, 0.0), corner(0.0, 8.0)],
            }]),
        }],
    }
}
