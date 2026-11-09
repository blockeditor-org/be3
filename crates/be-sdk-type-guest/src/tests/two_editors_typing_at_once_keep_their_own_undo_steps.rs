use super::*;

#[test]
fn two_editors_typing_at_once_keep_their_own_undo_steps() {
    let mut sim = Sim::<ChecklistContent>::new(1, checklist_module);
    sim.open(0);
    sim.attach(0, 1, true);
    sim.attach(0, 2, true);
    let (item, created) = Checklist::add("a");
    sim.operate(0, 1, created);
    sim.advance(1_000);
    sim.operate(0, 1, Checklist::set_text(item, "b"));
    sim.advance(100);
    sim.operate(0, 2, Checklist::set_text(item, "c"));
    sim.advance(100);
    sim.operate(0, 1, Checklist::set_text(item, "d"));
    sim.settle();

    let mut seen = Vec::new();
    while sim.change(0, Change::Undo) {
        seen.push(texts(&sim.content(0)));
    }
    assert_eq!(
        seen,
        [
            vec!["c".to_owned()],
            vec!["b".to_owned()],
            vec!["a".to_owned()],
            vec![]
        ],
        "an edit joins only a burst of the same editor's"
    );
}
