use super::*;

#[test]
fn picking_a_node_in_the_components_tab_selects_the_component_that_built_it() {
    let (document, [face]) = toolbar_of(|| {
        [view! {
            <ButtonFace label="Save" />
        }]
    });
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.toggle_inspector();
    harness.show_inspector_tab(crate::inspector::InspectorTab::Components);
    harness.frame(Vec::new());

    harness.toggle_picking();
    let face_rect = harness
        .document()
        .node_rect(face)
        .expect("the face was not laid out");
    harness.click(pos2(face_rect.left() + 2.0, face_rect.top() + 2.0));
    harness.frame(Vec::new());

    assert_eq!(harness.inspector().state().selected.get(), Some(face));
    assert_eq!(
        harness.selected_row().as_deref(),
        Some("beui::tests::ButtonFace")
    );

    harness.click(harness.row_center(3));
    harness.frame(Vec::new());

    assert_eq!(harness.inspector().state().selected.get(), Some(face));
    assert_eq!(harness.selected_row().as_deref(), Some("primitives::Frame"));
}
