use super::*;
use crate::reactive::view;

#[test]
fn picking_a_node_in_the_components_tab_selects_the_component_that_built_it() {
    let (document, [face]) = toolbar_of(|| {
        [view! {
            <ButtonFace label="Save" />
        }]
    });
    let mut harness = Harness::new(document);
    harness.toggle_inspector();
    harness.click(harness.components_tab_center());
    harness.frame(Vec::new());

    harness.toggle_picking();
    let face_rect = harness
        .document
        .node_rect(face)
        .expect("the face was not laid out");
    harness.click(pos2(face_rect.left() + 2.0, face_rect.top() + 2.0));
    harness.frame(Vec::new());

    assert_eq!(harness.inspector().state.selected.get(), Some(face));
    assert_eq!(harness.selected_row().as_deref(), Some("Frame"));

    harness.click(harness.row_center(1));
    harness.frame(Vec::new());

    assert_eq!(harness.inspector().state.selected.get(), Some(face));
    assert_eq!(harness.selected_row().as_deref(), Some("ButtonFace"));
}
