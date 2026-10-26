use super::*;

#[test]
fn a_gesture_is_one_undo_step_and_quick_edits_absorb() {
    let mut sim = Sim::<ChecklistContent>::new(1, checklist_module);
    sim.open(0);
    sim.attach(0, 1, true);
    let (item, created) = Checklist::add("a");
    sim.operate(0, 1, created);
    sim.advance(1_000);
    sim.operate(0, 1, Checklist::set_text(item, "b"));
    sim.advance(100);
    sim.operate(0, 1, Checklist::set_text(item, "c"));
    sim.advance(1_000);
    sim.operate(0, 1, Checklist::set_text(item, "d"));
    sim.advance(1_000);
    sim.gesture(0, 1, Gesture::Begin);
    sim.operate(0, 1, Checklist::set_text(item, "e"));
    sim.advance(5_000);
    sim.operate(0, 1, Checklist::set_text(item, "f"));
    sim.gesture(0, 1, Gesture::Commit);
    sim.settle();
    assert_eq!(texts(&sim.content(0)), ["f"]);
    let mut seen = Vec::new();
    while sim.change(0, Change::Undo) {
        seen.push(texts(&sim.content(0)));
    }
    assert_eq!(
        seen,
        [
            vec!["d".to_owned()],
            vec!["c".to_owned()],
            vec!["a".to_owned()],
            vec![]
        ]
    );
    assert_eq!(texts(sim.mirror(0, 1)), Vec::<String>::new());
    sim.gesture(0, 1, Gesture::Begin);
    let (_, created) = Checklist::add("cancelled");
    sim.operate(0, 1, created);
    sim.gesture(0, 1, Gesture::Cancel);
    sim.settle();
    assert!(texts(&sim.content(0)).is_empty());
    assert!(!sim.devices[0].history.0);
}
