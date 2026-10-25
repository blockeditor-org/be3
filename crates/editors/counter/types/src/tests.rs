use be_block::{BlockContent, LiveEdit, Merge, Undo};
use be_commit::MergeResult;

use crate::{Counter, CounterContent};

mod a_counter_reset_undoes_back_to_its_count;
mod a_reset_racing_an_increment_keeps_the_increment;
mod two_counters_merge_by_keeping_both_sides_of_the_count;
mod undoing_a_reset_restores_only_what_was_reset;

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
