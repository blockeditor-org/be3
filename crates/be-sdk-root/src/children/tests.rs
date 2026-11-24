use be_lib_geometry::{Rect, pos2};
use be_sdk_api::{ChildContent, ChildId, ChildLayer, ChildMode, ChildPlacement, TopBar};

use super::*;

mod a_child_is_asked_for_its_place_at_its_placers_raster;
mod a_child_kept_placed_keeps_its_editor_and_one_placed_no_longer_goes;
mod a_child_takes_the_presses_only_where_it_shows;
mod a_placer_is_hit_everywhere_but_where_its_children_that_take_input_are;

/// A block child placed at `rect` in its placer's view, `mode`, under it.
fn placed(rect: Rect, mode: ChildMode) -> ChildPlacement {
    ChildPlacement {
        child: ChildId(7),
        content: ChildContent::Block {
            block_id: [1; 16],
            block_type: [2; 16],
            view_block: None,
            editor: None,
        },
        rect,
        clip: rect,
        own_frame: false,
        top_bar: TopBar::Hidden,
        corner_radius: 0.0,
        layer: ChildLayer::Below,
        mode,
        intrinsic: None,
        fit: None,
        rotation: 0.0,
        opacity: 1.0,
        follows_view: false,
        dialog: None,
        docks: None,
        anchor: None,
        fullscreen: false,
        keyboard_ask: 0,
    }
}

/// The child `placement` places in editor 1's main view, as `settle` takes it in.
fn wanted(placement: ChildPlacement) -> Child {
    Child::placed(EditorInstanceId(1), ViewId::MAIN, 1.0, placement)
        .expect("a block child placed to run its editor is shown")
}

fn rect(min: (f32, f32), max: (f32, f32)) -> Rect {
    Rect::from_min_max(pos2(min.0, min.1), pos2(max.0, max.1))
}
