use be_block::{BlockContent, LiveEdit, Merge};

mod a_map_keeps_its_points_in_bounds_and_follows_its_children;

fn edited<C: LiveEdit + Clone>(start: &C, edits: impl IntoIterator<Item = C::Op>) -> C {
    let mut content = start.clone();
    for edit in edits {
        content.apply(&edit);
    }
    content
}
