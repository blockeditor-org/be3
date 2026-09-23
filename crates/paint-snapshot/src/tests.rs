use std::collections::BTreeMap;

use crate::{Content, Frame, Primitive, Snapshot, Texture, Triangle, Vertex};

mod a_frame_that_changed_is_named_by_its_number;
mod a_glyph_paints_its_coverage_in_its_colour;
mod a_recording_keeps_the_frames_it_was_given;
mod a_rounded_rect_is_covered_the_way_the_shader_covers_it;
mod a_snapshot_survives_a_round_trip;
mod a_triangle_is_filled_with_its_corner_colour;
mod a_turned_rounded_rect_is_covered_where_it_turned_to;

fn white() -> Texture {
    Texture::encode([1, 1], &[[255, 255, 255, 255]]).unwrap()
}

fn triangle(colour: [u8; 4]) -> Snapshot {
    Snapshot::of(frame(colour), BTreeMap::from([(0, white())]))
}

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
            content: Content::Mesh(vec![Triangle {
                texture: 0,
                corners: [corner(0.0, 0.0), corner(8.0, 0.0), corner(0.0, 8.0)],
            }]),
        }],
    }
}

fn triangles(colours: &[[u8; 4]]) -> Snapshot {
    Snapshot {
        frames: colours.iter().map(|colour| frame(*colour)).collect(),
        textures: BTreeMap::from([(0, white())]),
    }
}
