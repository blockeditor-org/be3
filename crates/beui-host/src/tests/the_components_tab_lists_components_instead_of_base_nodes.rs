use super::*;

#[test]
fn the_components_tab_lists_components_instead_of_base_nodes() {
    let (document, [_face]) = toolbar_of(|| {
        [view! {
            <ButtonFace label="Save" />
        }]
    });
    let mut harness = Harness::new(document);

    harness.toggle_inspector();
    assert_eq!(harness.tree(), ["list", "  frame", "    text"]);

    harness.show_inspector_tab(crate::inspector::InspectorTab::Components);
    harness.frame(Vec::new());

    assert_eq!(
        harness.tree(),
        [
            "primitives::List",
            "  beui_host::tests::ButtonFace",
            "    internals",
            "      primitives::Frame",
            "        primitives::Text",
            "          \"Save\""
        ]
    );
}
