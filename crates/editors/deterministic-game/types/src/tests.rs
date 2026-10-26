use be_block::{BlockContent, LiveEdit, Merge};

mod moves_played_on_both_sides_merge_to_both;

fn edited<C: LiveEdit + Clone>(start: &C, edits: impl IntoIterator<Item = C::Op>) -> C {
    let mut content = start.clone();
    for edit in edits {
        content.apply(&edit);
    }
    content
}
