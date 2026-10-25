use be_block::LiveEdit;

mod resetting_a_ray_traced_scene_clears_pixels_and_entities;

fn edited<C: LiveEdit + Clone>(start: &C, edits: impl IntoIterator<Item = C::Op>) -> C {
    let mut content = start.clone();
    for edit in edits {
        content.apply(&edit);
    }
    content
}
