use super::*;

#[test]
fn points_scored_on_both_sides_of_a_merge_add_up() {
    let (ada, add_ada) = Scoreboard::add("Ada");
    let base = edited(&ScoreboardContent::default(), [add_ada]);
    let ours = edited(&base, [Scoreboard::score(ada, 2)]);
    let theirs = edited(&base, [Scoreboard::score(ada, 5)]);

    let MergeResult::Clean(merged) = ScoreboardContent::merge3(&base, &ours, &theirs) else {
        panic!("adding to a score never conflicts");
    };

    assert_eq!(merged.root().players[0].score.get(), 7);
}
