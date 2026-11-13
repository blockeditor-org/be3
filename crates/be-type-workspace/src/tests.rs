use be_block_content::{BlockContent, LiveEdit, Merge};
use be_block_commit::MergeResult;
use uuid::Uuid;

mod a_block_added_to_a_folder_by_both_sides_is_listed_once;
mod a_folder_lists_each_block_once_and_follows_its_children;
mod a_folder_reorders_a_child_after_a_sibling_or_first;
mod a_workspace_reorders_its_children_like_a_folder;

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
