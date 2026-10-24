use be_block::{BlockContent, LiveEdit, Merge};
use be_commit::MergeResult;

use crate::{BrowserTabContent, HistoryItem};

mod a_browser_tab_is_named_after_its_page;
mod a_browser_tab_push_discards_forward_history;
mod pages_opened_on_both_sides_are_both_kept_in_history;

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
