use be_block::{LiveEdit, Merge, Undo};
use be_commit::MergeResult;

use be_model::Edit;

mod a_pixel_painted_outside_the_other_sides_crop_counts_as_a_conflict;
mod pixel_art_fills_resizes_and_keeps_a_paint_made_during_a_resize;
mod undoing_a_clear_keeps_a_pixel_painted_since;

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

fn undone<C: Undo>(content: &mut C, operation: C::Op) -> C::Step {
    let step = content
        .step(&operation)
        .expect("the operation changes something");
    content.apply(&operation);
    step
}

fn reverted<C: Undo>(content: &mut C, step: &C::Step) {
    for operation in content.revert(step) {
        content.apply(&operation);
    }
}
