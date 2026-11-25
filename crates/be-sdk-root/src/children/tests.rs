use be_lib_geometry::{Rect, pos2};
use be_sdk_api::{ChildContent, ChildId, ChildLayer, ChildMode, ChildPlacement, TopBar};

use super::*;

mod a_block_placed_in_several_places_gets_an_editor_in_each;
mod a_child_in_a_dock_tabs_body_is_docked_before_its_editor_is_first_asked_for_screens;
mod a_child_is_asked_for_its_place_at_its_placers_raster;
mod a_child_kept_placed_keeps_its_editor_and_one_placed_no_longer_goes;
mod a_child_placed_in_a_dock_tabs_body_joins_its_placers_dock;
mod a_child_placed_to_fit_its_content_is_asked_for_at_most_that_room;
mod a_child_placed_with_an_intrinsic_size_is_laid_out_at_it_and_drawn_at_its_zoom;
mod a_child_takes_the_presses_only_where_it_shows;
mod a_child_taking_its_placers_frame_is_asked_for_that_frame_with_its_place_as_content;
mod a_child_taking_the_frame_is_hit_but_where_its_placers_world_shows;
mod a_child_that_opens_fails_or_is_refused_takes_no_frame;
mod a_childs_dock_views_keep_their_screens_while_placed;
mod a_dialog_child_runs_in_an_editor_of_its_own;
mod a_dialog_in_a_view_anchored_to_an_edge_opens_as_a_sheet;
mod a_joined_docks_body_is_its_owners_dock_view_framed_where_it_holds_the_content;
mod a_keyboard_ask_for_a_child_not_up_takes_the_keyboard_out_and_gives_it_once_up;
mod a_keyboard_ask_never_takes_the_keyboard_from_outside_its_editor;
mod a_new_keyboard_ask_moves_the_keyboard_into_a_child_that_shows;
mod a_placer_is_hit_everywhere_but_where_its_children_that_take_input_are;
mod a_placers_occluders_take_the_presses_over_its_children;
mod a_press_outside_a_child_taking_the_frame_takes_the_frame_back;
mod a_press_outside_an_active_child_takes_it_back_and_escape_does_not;
mod a_preview_shows_its_editors_preview_view_and_keeps_its_editor_once_promoted;
mod a_program_window_is_a_host_layer_at_its_place_as_big_as_its_place;
mod a_turned_child_is_drawn_turned_about_the_middle_of_its_place;
mod a_view_holding_the_editors_content_is_the_one_whose_frame_a_child_takes;
mod each_placer_above_a_child_hears_the_keyboard_went_into_it;
mod only_an_active_or_live_child_with_no_frame_of_its_own_takes_a_framed_view;
mod taking_the_frame_back_takes_it_from_every_child_of_that_view;

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
    Child::placed(
        EditorInstanceId(1),
        ViewId::MAIN,
        1.0,
        placement,
        Vec::new(),
        false,
    )
    .expect("a block child placed to run its editor is shown")
}

/// The layer showing editor 1's main view, in app 5.
const PLACER: LayerId = LayerId(100);

/// Children of app 5, whose editor 1's main view `PLACER` shows, placing `shown`.
fn placing(shown: Vec<Child>) -> Children {
    Children {
        instance: 5,
        placers: vec![(PLACER, EditorInstanceId(1), ViewId::MAIN)],
        shown,
        ..Children::default()
    }
}

/// A child `id` of editor 1 showing block `[id; 16]`, placed `mode`, whose app is up and shows
/// in the layer `200 + id`.
fn up(id: u64, mode: ChildMode) -> Child {
    let mut placement = placed(rect((0.0, 0.0), (100.0, 100.0)), mode);
    placement.child = ChildId(id);
    if let ChildContent::Block { block_id, .. } = &mut placement.content {
        *block_id = [id as u8; 16];
    }
    let mut child = wanted(placement);
    child.layer = Some(LayerId(200 + id));
    child.up = true;
    child
}

/// Editor 1's placements as a settle hears them, asking with `serial` that `child` take the
/// keyboard.
fn ask(children: &mut Children, child: u64, serial: u64) {
    let mut asking = HashMap::default();
    asking.insert(EditorInstanceId(1), (serial, ChildId(child)));
    children.hear_asks(asking);
}

/// Where the children's asks move the keyboard, with it in `focused` and every layer drawn but
/// `hidden`.
fn keyboard(
    children: &mut Children,
    focused: Option<LayerId>,
    hidden: &[LayerId],
) -> Option<KeyboardMove> {
    children.keyboard(focused, &|layer| !hidden.contains(&layer))
}

fn rect(min: (f32, f32), max: (f32, f32)) -> Rect {
    Rect::from_min_max(pos2(min.0, min.1), pos2(max.0, max.1))
}
