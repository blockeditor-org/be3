use uuid::Uuid;

use crate::{CELLS, GEMS, Hunt, HuntContent};

mod a_cell_counts_the_gems_around_it;
mod a_new_world_hides_its_gems_again_and_fills_every_cell_in;
mod a_world_is_over_once_every_gem_is_dug_up;

fn edited(start: &HuntContent, edits: impl IntoIterator<Item = be_block_model::Edit>) -> HuntContent {
    let mut content = start.clone();
    for edit in edits {
        content.apply(&edit);
    }
    content
}
