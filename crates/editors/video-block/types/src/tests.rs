use be_block::{BlockContent, LiveEdit, Merge};
use be_commit::MergeResult;
use uuid::Uuid;

mod clips_attached_to_each_other_on_each_side_merge_without_a_cycle;
mod video_clips_attach_ripple_and_refuse_cycles;

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
