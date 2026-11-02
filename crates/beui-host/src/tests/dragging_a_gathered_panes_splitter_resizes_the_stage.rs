use super::*;

#[test]
fn dragging_a_gathered_panes_splitter_resizes_the_stage() {
    let _root = crate::reactive::RootScope::new();
    let (document, layout) = staged_dock();
    let mut harness = Harness::sized(document, PHONE_VIEWPORT);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let stage = harness.rect(harness.find("content.1"));
    let bar = harness.rect(harness.find("bar.9"));
    assert!(
        stage.contains_rect(bar) && (bar.bottom() - stage.bottom()).abs() < 40.0,
        "the stage's bar floats over the bottom of the stage, {bar:?} {stage:?}"
    );
    let splitter = harness.center(harness.find("dock.compact.splitter"));

    harness.drag(splitter, splitter - vec2(0.0, 100.0));
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let shorter = harness.rect(harness.find("content.1"));
    let gathered = harness.rect(harness.find("dock.compact"));
    assert!(
        (shorter.height() - (stage.height() - 100.0)).abs() < 2.0,
        "dragging the splitter up takes the stage's room, {shorter:?}"
    );
    assert!((shorter.bottom() - gathered.top()).abs() < 2.0);
    let moved = harness.rect(harness.find("bar.9"));
    assert!(
        shorter.contains_rect(moved) && (moved.top() - (bar.top() - 100.0)).abs() < 2.0,
        "the stage's bar moves up with it, {moved:?}"
    );

    let saved = layout.snapshot();
    let compact = saved.state().compacts().to_vec();
    assert_eq!(compact.len(), 1, "the dragged size is kept with the layout");
    assert!(
        compact[0].portrait.is_some() && compact[0].landscape.is_none(),
        "{compact:?}"
    );
    let tiled = saved.state().spec().main;
    harness.resize(WIDE_VIEWPORT);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    assert_eq!(
        layout.snapshot().state().spec().main,
        tiled,
        "the compact size leaves the tiled layout as it was"
    );
}
