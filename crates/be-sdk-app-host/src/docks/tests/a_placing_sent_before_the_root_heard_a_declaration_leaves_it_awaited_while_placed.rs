use super::*;

#[test]
fn a_placing_sent_before_the_root_heard_a_declaration_leaves_it_awaited_while_placed() {
    let mut docks = nested();
    host_seen(&mut docks, DockHold::Placing, 0);
    docks.placed(ROOT, &[], None);
    assert!(
        docks.docked(OUTER),
        "the root stops placing the child to host the dock it was sent, and the child stays"
    );
    host(&mut docks, DockHold::Hosted(hosted_by_root()));
    assert_eq!(
        docks.hosting.get(&(ROOT, OUTER_CHILD)),
        Some(&Hosting::Hosted),
        "and its answer to that declaration lands"
    );
}
