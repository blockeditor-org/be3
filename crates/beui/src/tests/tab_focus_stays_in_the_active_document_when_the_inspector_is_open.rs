use super::*;
use crate::reactive::*;

#[test]
fn tab_focus_stays_in_the_active_document_when_the_inspector_is_open() {
    let (document, [first, second]) = toolbar_of(|| {
        [
            view! {
                <LabelledButton label="First" />
            },
            view! {
                <LabelledButton label="Second" />
            },
        ]
    });
    let first_focused = document.api::<unstyled::ButtonApi>(first).focused();
    let second_focused = document.api::<unstyled::ButtonApi>(second).focused();
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);

    harness.toggle_inspector();
    harness.key(Key::Tab, Modifiers::NONE);

    assert_eq!((first_focused.get(), second_focused.get()), (true, false));
    assert_eq!(harness.inspector().document.focused_node(), None);

    let accesskit_tab = harness.accesskit_tab_center();
    harness.click(accesskit_tab);
    harness.frame(Vec::new());
    harness.click(harness.row_center(0));
    let inspector_focus = harness.inspector().document.focused_node();
    assert!(inspector_focus.is_some());
    assert_eq!(harness.document().focused_node(), None);

    harness.key(Key::Tab, Modifiers::NONE);

    assert_eq!(harness.document().focused_node(), None);
    assert_ne!(harness.inspector().document.focused_node(), inspector_focus);
}
