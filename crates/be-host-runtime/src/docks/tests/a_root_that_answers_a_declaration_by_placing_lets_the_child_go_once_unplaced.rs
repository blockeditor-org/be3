use super::*;

#[test]
fn a_root_that_answers_a_declaration_by_placing_lets_the_child_go_once_unplaced() {
    let mut docks = nested();
    host(&mut docks, DockHold::Placing);
    assert!(docks.docked(OUTER), "placed, the child stays");
    docks.placed(ROOT, &[], None);
    assert!(
        !docks.docked(OUTER),
        "a root that heard the declaration and kept placing the child lets it go when it stops"
    );
    assert!(
        !docks.hosting.contains_key(&(ROOT, OUTER_CHILD)),
        "nothing of it is left awaiting an answer"
    );
}
