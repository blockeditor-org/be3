use super::*;
use crate::primitives::Frame;
use crate::reactive::*;
use crate::styled::*;
use crate::primitives::List;

#[test]
fn opening_a_menu_button_damages_only_the_button_and_its_menu() {
    let _root = crate::reactive::RootScope::new();
    let button = NodeRef::new();
    let document = build_node(move || {
        let items = view! {
            <unstyled::MenuItem label="Copy" />
            <unstyled::MenuItem label="Paste" />
        };
        view! {
            <Frame fill={Color32::from_gray(20)} radius=0.0 padding=16.0>
                <List spacing=0.0>
                    <MenuButton
                        @node_ref=&button
                        label="Actions"
                        items
                        on_select={|_: Vec<usize>| {}}
                    />
                </List>
            </Frame>
        }
    });
    let button = button.get();
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let pos = harness.center(button);
    harness.press_at(pos);
    let opened = harness.frame(vec![Input::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed: false,
    }]);

    let damage = opened.main().damage().expect("opening a menu repaints");
    assert!(
        damage.min.y > pos.y,
        "the button looks pressed from the press until its menu closes, \
         so only the menu under it is damaged: {damage:?}"
    );
    assert!(
        damage.height() < VIEWPORT.y * 0.5,
        "the background painted under the whole window did not change, \
         so the menu appearing over it must not damage all of it: {damage:?}"
    );
}
