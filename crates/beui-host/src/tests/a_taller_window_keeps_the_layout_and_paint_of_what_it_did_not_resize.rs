use super::*;

#[test]
fn a_taller_window_keeps_the_layout_and_paint_of_what_it_did_not_resize() {
    let _root = crate::reactive::RootScope::new();
    let panels = three_panels();
    let middle = panels.middle;
    let mut document = panels.document;
    let (layouts, _) = counted(&mut document, middle);
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let settled = layouts.get_untracked();

    harness.resize(harness.size() + vec2(0.0, 100.0));
    harness.frame(Vec::new());
    assert_eq!(
        layouts.get_untracked(),
        settled,
        "a panel the taller window gives the same size must keep its placement"
    );
    let work = harness.document().performance().latest.work;
    assert!(work.reused_placements > 0, "got {work:?}");
    assert!(work.replayed_nodes > 0, "got {work:?}");

    harness.resize(harness.size() + vec2(100.0, 0.0));
    harness.frame(Vec::new());
    assert!(
        layouts.get_untracked() > settled,
        "a panel the wider window widens has to be laid out again"
    );
}
