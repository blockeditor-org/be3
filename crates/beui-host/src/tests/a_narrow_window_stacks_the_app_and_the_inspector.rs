use super::*;

#[test]
fn a_narrow_window_stacks_the_app_and_the_inspector() {
    let _root = crate::reactive::RootScope::new();
    let HelloColumn { document, .. } = hello_column();
    let mut harness = Harness::new(document);

    harness.toggle_inspector();
    harness.settle();

    assert!(harness.inspector().tools_stacked());
    assert_eq!(harness.inspector_panel_rect().width(), VIEWPORT.x);
    assert!(
        !harness.inspector().app_visible(),
        "the inspector opens on its tools, which cover the app"
    );
}
