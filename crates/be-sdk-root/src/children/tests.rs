use be_lib_geometry::{Rect, pos2};
use be_sdk_api::{ChildContent, ChildId, ChildLayer, ChildMode, ChildPlacement, TopBar};
use be_wasm_root_api::{Gift, GiftTo, PlacerEvent};

use super::*;

mod a_child_is_asked_for_its_place_at_its_placers_raster;
mod a_child_kept_placed_keeps_its_editor_and_one_placed_no_longer_goes;
mod a_child_takes_the_presses_only_where_it_shows;
mod a_gift_waits_for_its_child_to_be_up_and_counts_from_there;
mod a_gift_to_a_child_that_went_is_missed_and_one_not_placed_yet_waits;
mod a_press_outside_an_active_child_or_escape_in_its_placer_takes_it_back;
mod each_placer_above_a_child_hears_the_keyboard_went_into_it;
mod the_latest_gift_of_an_editor_stands_and_asking_again_keeps_its_count;
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
    }
}

/// The child `placement` places in editor 1's main view, as `settle` takes it in.
fn wanted(placement: ChildPlacement) -> Child {
    Child::placed(EditorInstanceId(1), ViewId::MAIN, 1.0, placement)
        .expect("a block child placed to run its editor is shown")
}

/// The layer showing editor 1's main view, in app 5.
const PLACER: LayerId = LayerId(100);

/// Children of app 5, whose editor 1's main view `PLACER` shows, placing `shown`, which are
/// placed now, with editor 1 having made `commits` layout commits.
fn placing(shown: Vec<Child>, commits: u32) -> Children {
    let placed = shown
        .iter()
        .map(|child| (child.editor, child.child))
        .collect();
    let mut children = Children {
        instance: 5,
        placers: vec![(PLACER, EditorInstanceId(1), ViewId::MAIN)],
        shown,
        ..Children::default()
    };
    children.placed = placed;
    children.commits.insert(EditorInstanceId(1), commits);
    children
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

fn gift(instance: u64, editor: u64, to: GiftTo) -> Gift {
    Gift {
        instance,
        editor,
        to,
    }
}

/// Gives what the children's gifts can at `now`, with the keyboard in `focused` and every layer
/// drawn but `hidden`: the layer given, and what the placers were told.
fn give(
    children: &mut Children,
    focused: LayerId,
    hidden: &[LayerId],
    now: Duration,
) -> (Option<LayerId>, Vec<(u64, u64, PlacerEvent)>) {
    let mut told = Vec::new();
    let given = children.give(
        Some(focused),
        &|layer| !hidden.contains(&layer),
        now,
        &mut told,
    );
    (given, told)
}

fn rect(min: (f32, f32), max: (f32, f32)) -> Rect {
    Rect::from_min_max(pos2(min.0, min.1), pos2(max.0, max.1))
}
