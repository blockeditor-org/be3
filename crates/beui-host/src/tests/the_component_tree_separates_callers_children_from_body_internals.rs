use super::*;
use crate::reactive::*;

#[component]
fn Shell(children: Children<ListChild>) -> NodeId {
    view! {
        <primitives::List spacing=0.0>
            <styled::Text>"Inside"</styled::Text>
            {children}
        </primitives::List>
    }
}

#[test]
fn the_component_tree_separates_callers_children_from_body_internals() {
    let document = build_node(|| {
        view! {
            <Shell>
                <styled::Text>"Outside"</styled::Text>
                <styled::Text>"Sibling"</styled::Text>
            </Shell>
        }
    });
    let tree = document.component_tree();
    let public = tree.text(&document, false);
    assert_eq!(
        public,
        "beui_host::tests::the_component_tree_separates_callers_children_from_body_internals::Shell\n  styled::Text\n    \"Outside\"\n  styled::Text\n    \"Sibling\"\n"
    );
    let full = tree.text(&document, true);
    assert!(
        full.contains("  internals\n    primitives::List\n"),
        "{full}"
    );
    assert!(full.contains("      styled::Text\n        internals\n          primitives::Text\n        \"Inside\"\n"), "{full}");
    let mut harness = Harness::new(document);
    harness.toggle_inspector();
    harness.show_inspector_tab(crate::inspector::InspectorTab::Components);
    harness.frame(Vec::new());
    assert_eq!(
        harness.tree(),
        [
            "beui_host::tests::the_component_tree_separates_callers_children_from_body_internals::Shell",
            "  internals",
            "    primitives::List",
            "      styled::Text",
            "        internals",
            "        \"Inside\"",
            "  styled::Text",
            "    internals",
            "    \"Outside\"",
            "  styled::Text",
            "    internals",
            "    \"Sibling\"",
        ]
    );
    harness.click(harness.chevron_center(1));
    harness.frame(Vec::new());
    assert_eq!(
        harness.tree(),
        [
            "beui_host::tests::the_component_tree_separates_callers_children_from_body_internals::Shell",
            "  internals",
            "  styled::Text",
            "    internals",
            "    \"Outside\"",
            "  styled::Text",
            "    internals",
            "    \"Sibling\"",
        ]
    );
}
