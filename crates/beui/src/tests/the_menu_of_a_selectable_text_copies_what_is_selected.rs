use super::*;
use crate::reactive::{List, NodeRef, Text, build, view};
use crate::styled::SelectableText;

#[test]
fn the_menu_of_a_selectable_text_copies_what_is_selected() {
    let (first, second, region) = (NodeRef::new(), NodeRef::new(), NodeRef::new());
    let document = build({
        let (first, second, region) = (first.clone(), second.clone(), region.clone());
        move || {
            view! {
                <SelectableText @node_ref=&region>
                    <List spacing=4.0>
                        <Text @node_ref=&first string="main" />
                        <Text @node_ref=&second string="0123abc" />
                    </List>
                </SelectableText>
            }
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let (first, second) = (harness.rect(first.get()), harness.rect(second.get()));

    harness.drag(
        pos2(first.left() + 1.0, first.center().y),
        pos2(second.right() - 1.0, second.center().y),
    );
    let at = pos2(first.left() + 4.0, first.center().y);
    harness.frame(vec![Event::PointerButton {
        pos: at,
        button: PointerButton::Secondary,
        pressed: true,
        modifiers: Modifiers::NONE,
    }]);
    harness.frame(vec![Event::PointerButton {
        pos: at,
        button: PointerButton::Secondary,
        pressed: false,
        modifiers: Modifiers::NONE,
    }]);
    let menu = unstyled::context_menu_menu(harness.document(), region.get());
    let copy = harness.center(unstyled::menu_list_row_button(harness.document(), menu, 0));
    harness.frame(vec![Event::PointerMoved(copy)]);
    harness.frame(vec![Event::PointerButton {
        pos: copy,
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    }]);
    let clicked = harness.frame(vec![Event::PointerButton {
        pos: copy,
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    }]);

    assert_eq!(
        clicked.copied_text.as_deref(),
        Some("main\n0123abc"),
        "Copy is the first item of the menu"
    );
}
