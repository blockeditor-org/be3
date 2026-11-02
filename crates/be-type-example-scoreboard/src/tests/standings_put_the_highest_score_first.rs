use super::*;

#[test]
fn standings_put_the_highest_score_first() {
    let (ada, add_ada) = Scoreboard::add("Ada");
    let (bob, add_bob) = Scoreboard::add("Bob");
    let (cy, add_cy) = Scoreboard::add("Cy");
    let content = edited(
        &ScoreboardContent::default(),
        [
            add_ada,
            add_bob,
            add_cy,
            Scoreboard::score(bob, 3),
            Scoreboard::score(cy, 3),
        ],
    );

    assert_eq!(content.root().standings(), [bob, cy, ada]);

    let reset = edited(&content, [content.root().reset()]);
    assert_eq!(reset.root().standings(), [ada, bob, cy]);
}
