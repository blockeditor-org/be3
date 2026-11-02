use be_block::Merge;
use be_block_commit::MergeResult;

use crate::{Scoreboard, ScoreboardContent};

mod points_scored_on_both_sides_of_a_merge_add_up;
mod standings_put_the_highest_score_first;

fn edited(
    start: &ScoreboardContent,
    edits: impl IntoIterator<Item = be_block_model::Edit>,
) -> ScoreboardContent {
    let mut content = start.clone();
    for edit in edits {
        content.apply(&edit);
    }
    content
}
