use be_block::{LiveEdit, Merge, Undo};
use be_commit::MergeResult;
use be_model::ObjectId;

use crate::{Checklist, ChecklistContent};

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

mod checks_of_different_items_at_once_both_stick;
mod clearing_a_checklist_keeps_the_open_items;
mod clearing_done_items_removes_one_someone_reopened_at_once;
mod clearing_done_items_while_the_other_side_renames_one_removes_it;
mod items_added_by_two_peers_at_once_are_both_kept;
mod two_checklists_merge_every_item_either_side_added;
mod undoing_a_clear_puts_the_items_back_in_order;
