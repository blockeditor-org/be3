use super::*;
use crate::reactive::view;

#[test]
fn the_components_tab_lists_components_instead_of_base_nodes() {
    let (document, [_face]) = toolbar_of(|| {
        [view! {
            <ButtonFace label="Save" />
        }]
    });
    let mut harness = Harness::new(document);

    harness.toggle_inspector();
    assert_eq!(harness.tree(), ["column", "  frame", "    text"]);

    harness.click(harness.components_tab_center());
    harness.frame(Vec::new());

    assert_eq!(
        harness.tree(),
        ["List", "  ButtonFace", "    Frame", "      Text"]
    );
}
