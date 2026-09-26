use block_editor_plugin::be_block::{BlockContent, TriangleContent};
use block_editor_plugin::{
    EditorHost, EditorRegion, FrameSpec, GraphCommand, Instance, Rect, Region, pos2, vec2,
};

use crate::app::Triangle;

mod a_triangle_asks_to_be_painted_until_it_has_been;
mod creating_a_triangle_makes_a_block_with_no_state;

fn region() -> Region {
    Region {
        region: EditorRegion::Frame,
        rect: Rect::from_min_size(pos2(0.0, 0.0), vec2(400.0, 300.0)),
        scale_factor: 1.0,
        spec: FrameSpec::default(),
    }
}
