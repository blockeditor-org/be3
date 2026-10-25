use be_block::{BlockContent, LiveEdit, Merge};
use be_commit::MergeResult;

use crate::{TextBlock, TextContent, TextLanguage};

mod a_language_changed_on_both_sides_counts_a_conflict;
mod a_text_file_becomes_a_text_block_and_back;
mod content_is_stamped_with_the_shape_it_was_saved_in;
mod frozen_text;
mod inserts_at_one_place_from_two_peers_keep_each_peers_text_whole;
mod text_edited_in_different_paragraphs_merges_cleanly;
mod text_merges_line_by_line_and_marks_real_conflicts;
mod text_reads_every_register_without_any_one_field;
mod text_saved_before_it_was_frozen_still_loads;
mod text_typed_inside_a_range_someone_deletes_at_once_survives;
mod the_same_range_deleted_by_two_peers_at_once_is_deleted_once;

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
