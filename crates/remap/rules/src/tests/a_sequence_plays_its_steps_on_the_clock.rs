use super::*;

#[test]
fn a_sequence_plays_its_steps_on_the_clock() {
    let mut engine = engine(
        r#"
        [[group]]
        name = "Cairn"
        rules = [{ on = "Q", steps = [{ tap = "Space", hold_ms = 40 }, { tap = "Q", hold_ms = 40 }] }]
        "#,
    );

    assert_eq!(engine.input(press("Q"), NOW), vec![press("Space")]);
    assert_eq!(engine.deadline(), Some(NOW + 40 * MS));
    assert_eq!(engine.input(release("Q"), NOW + 10 * MS), vec![]);
    assert_eq!(engine.timer(NOW + 39 * MS), vec![]);
    assert_eq!(
        engine.timer(NOW + 40 * MS),
        vec![release("Space"), press("Q")]
    );
    assert_eq!(engine.deadline(), Some(NOW + 80 * MS));
    assert_eq!(engine.timer(NOW + 80 * MS), vec![release("Q")]);
    assert_eq!(engine.deadline(), None);

    assert_eq!(
        engine.input(press("Q"), NOW + 100 * MS),
        vec![press("Space")],
        "a second press starts the sequence again"
    );
    assert_eq!(
        engine.input(press("Q"), NOW + 101 * MS),
        vec![],
        "the same press twice is one press"
    );
}
