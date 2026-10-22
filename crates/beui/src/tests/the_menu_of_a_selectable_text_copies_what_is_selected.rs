use super::*;
use crate::primitives::Text;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn the_menu_of_a_selectable_text_copies_what_is_selected() {
    let _root = crate::reactive::RootScope::new();
    let (first, second, region) = (NodeRef::new(), NodeRef::new(), NodeRef::new());
    let document = build_node(move || {
        view! {
            <SelectableText @node_ref=&region>
                <List spacing=4.0>
                    <Text @node_ref=&first>"main"</Text>
                    <Text @node_ref=&second>"0123abc"</Text>
                </List>
            </SelectableText>
        }
    });
    let mut harness = harness(document);
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
    let menu = harness
        .document()
        .api::<unstyled::ContextMenuApi>(region.get())
        .menu();
    let copy = harness.center(
        harness
            .document()
            .api::<unstyled::MenuListApi>(menu)
            .row_button(0),
    );
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
