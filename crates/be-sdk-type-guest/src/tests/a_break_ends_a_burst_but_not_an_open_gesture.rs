use super::*;

#[test]
fn a_break_ends_a_burst_but_not_an_open_gesture() {
    let mut sim = Sim::<ChecklistContent>::new(1, checklist_module);
    sim.open(0);
    sim.attach(0, 1, true);
    sim.attach(0, 2, true);
    let (item, created) = Checklist::add("a");
    sim.operate(0, 1, created);
    sim.advance(1_000);
    sim.operate(0, 1, Checklist::set_text(item, "b"));
    sim.advance(100);
    sim.gesture(0, 2, Gesture::Break);
    sim.operate(0, 1, Checklist::set_text(item, "c"));
    sim.advance(100);
    sim.gesture(0, 1, Gesture::Break);
    sim.advance(100);
    sim.operate(0, 1, Checklist::set_text(item, "d"));
    sim.advance(1_000);

    sim.gesture(0, 1, Gesture::Begin);
    sim.operate(0, 1, Checklist::set_text(item, "e"));
    sim.gesture(0, 1, Gesture::Break);
    sim.operate(0, 1, Checklist::set_text(item, "f"));
    sim.gesture(0, 1, Gesture::Cancel);
    sim.settle();
    assert_eq!(
        texts(&sim.content(0)),
        ["d"],
        "a break leaves the open gesture to its cancel, which takes back all of it"
    );

    let mut seen = Vec::new();
    while sim.change(0, Change::Undo) {
        seen.push(texts(&sim.content(0)));
    }
    assert_eq!(
        seen,
        [vec!["c".to_owned()], vec!["a".to_owned()], vec![]],
        "the editor's break ends its burst; another editor's leaves it be"
    );
}
