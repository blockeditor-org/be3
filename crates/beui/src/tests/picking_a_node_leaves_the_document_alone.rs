use super::*;
use crate::reactive::*;

#[test]
fn picking_a_node_leaves_the_document_alone() {
    let _root = crate::reactive::RootScope::new();
    let clicks = create_signal(0);
    let (document, [_button]) = toolbar_of(|| {
        [view! {
            <LabelledButton
                label="Click me"
                on_click={move || clicks.update(|clicks| *clicks += 1)}
            />
        }]
    });
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);

    harness.toggle_inspector();
    harness.toggle_picking();
    harness.settle();
    harness.click(harness.app_rect().min + vec2(4.0, 4.0));
    harness.frame(Vec::new());

    assert_eq!(clicks.get_untracked(), 0);
    assert!(harness.inspector().state().selected.get().is_some());
}
