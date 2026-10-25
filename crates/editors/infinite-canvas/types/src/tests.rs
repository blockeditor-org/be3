use be_block::{BlockContent, Edit, LiveEdit, Merge, Undo};
use be_commit::MergeResult;
use database_types::database::DatabaseValue;
use uuid::Uuid;

use crate::{
    Canvas, CanvasContent, CanvasEntity, CanvasPoint, CanvasShape, CanvasTransform,
    InfiniteCanvasOperation,
};

mod a_canvas_entity_removed_on_one_side_and_moved_on_the_other_stays_removed;
mod a_canvas_profile_is_kept_but_never_undone;
mod a_component_added_to_one_entity_on_both_sides_is_kept_once;
mod an_arrow_end_bound_to_a_shape_follows_it_and_stays_when_it_is_removed;
mod canvas_children_are_removed_repointed_or_merged;
mod canvas_layers_reorder_and_keep_unlisted_slots;
mod canvases_merge_a_move_and_a_restyle_of_one_entity;
mod entities_brought_forward_on_both_sides_merge_to_both_moves;
mod text_retyped_in_one_canvas_entity_by_two_peers_keeps_both;
mod two_peers_restyling_different_fields_of_one_shape_keep_both;

fn edited<C: LiveEdit + Clone>(start: &C, edits: impl IntoIterator<Item = C::Op>) -> C {
    let mut content = start.clone();
    for edit in edits {
        content.apply(&edit);
    }
    content
}

fn merged<C: Merge>(base: &C, ours: &C, theirs: &C) -> (C, usize) {
    match C::merge3(base, ours, theirs) {
        MergeResult::Clean(value) => (value, 0),
        MergeResult::Conflicted { value, conflicts } => (value, conflicts),
    }
}

fn rectangle() -> CanvasEntity {
    CanvasEntity::new(
        CanvasShape::Rectangle,
        CanvasTransform::new(CanvasPoint::default(), CanvasPoint::new(10.0, 10.0), 0.0),
    )
}

fn canvas_run(content: &CanvasContent, operation: InfiniteCanvasOperation) -> CanvasContent {
    edited(content, [Canvas::edit_in(content, &operation)])
}
