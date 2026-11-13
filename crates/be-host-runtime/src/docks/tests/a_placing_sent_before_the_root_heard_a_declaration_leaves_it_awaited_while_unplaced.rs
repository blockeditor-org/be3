use super::*;

fn placed_outer() -> [DockedChild; 1] {
    [DockedChild {
        child: OUTER_CHILD,
        tab: 100,
        block: OUTER_BLOCK,
    }]
}

#[test]
fn a_placing_sent_before_the_root_heard_a_declaration_leaves_it_awaited_while_unplaced() {
    let mut docks = nested();
    docks.placed(ROOT, &[], None);
    host_seen(&mut docks, DockHold::Placing, 0);
    docks.placed(ROOT, &placed_outer(), None);
    docks.placed(ROOT, &[], None);
    assert!(
        docks.docked(OUTER),
        "a hold sent before the declaration arrived leaves it awaited, whatever the root places"
    );
    host(&mut docks, DockHold::Hosted(hosted_by_root()));
    assert_eq!(
        docks.hosting.get(&(ROOT, OUTER_CHILD)),
        Some(&Hosting::Hosted)
    );
}
