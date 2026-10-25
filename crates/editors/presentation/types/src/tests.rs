use be_block::{BlockContent, LiveEdit, Merge, ObjectId};
use be_commit::MergeResult;
use uuid::Uuid;

mod a_presentation_moves_slides_by_index_and_follows_its_children;
mod slides_moved_by_two_peers_at_once_both_move;
mod slides_moved_on_both_sides_merge_to_both_moves;

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
