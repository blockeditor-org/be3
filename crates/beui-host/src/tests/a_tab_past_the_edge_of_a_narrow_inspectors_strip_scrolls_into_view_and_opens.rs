use super::*;
use crate::inspector::InspectorTab;

#[test]
fn a_tab_past_the_edge_of_a_narrow_inspectors_strip_scrolls_into_view_and_opens() {
    let _root = crate::reactive::RootScope::new();
    let document = build_node(move || view! { <Text color=Color32::WHITE>"Hello"</Text> });
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.toggle_inspector();
    harness.settle();
    let edge = harness.program_rect().right() + 0.5;
    harness.drag(pos2(edge, 300.0), pos2(edge + 120.0, 300.0));
    harness.settle();

    let inspector = harness.inspector();
    assert!(!inspector.tools_stacked(), "the tools keep their tab strip");
    let last = inspector.tab_node(InspectorTab::Drags);
    let rect = inspector.document.node_rect(last).expect("the tab is laid out");
    let shown = inspector.document.rects.visible(&last).expect("the tab is laid out");
    assert!(
        shown.right() < rect.right(),
        "the narrowed strip clips its last tab: {rect:?} shows {shown:?}"
    );

    harness.show_inspector_tab(InspectorTab::Drags);
    let inspector = harness.inspector();
    assert!(inspector.panel_rect().is_some(), "the inspector stays open");
    let last = inspector.tab_node(InspectorTab::Drags);
    assert_eq!(
        inspector.document.rects.visible(&last),
        inspector.document.node_rect(last),
        "the strip scrolled the whole tab into view"
    );
    assert!(inspector.tab_shown(InspectorTab::Drags), "the Drag tool shows");
    assert_eq!(inspector.drag_rows(), ["Nothing is carried"]);
}
