use super::*;
use crate::reactive::{Frame, NodeRef, build, view};
use crate::styled::MenuButton;

#[test]
fn opening_a_menu_button_damages_only_the_button_and_its_menu() {
    let button = NodeRef::new();
    let document = build({
        let button = button.clone();
        move || {
            let items = view! {
                <unstyled::MenuItem label="Copy" />
                <unstyled::MenuItem label="Paste" />
            };
            view! {
                <Frame
                    color={Color32::from_gray(20)}
                    radius=0
                    padding_horizontal=16.0
                    padding_vertical=16.0
                >
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
        }
    });
    let button = button.get();
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let pos = harness.center(button);
    harness.press_at(pos);
    let opened = harness.release_at(pos);

    let damage = opened.damage().expect("opening a menu repaints");
    assert!(
        damage.contains(pos),
        "the button that opened is damaged: {damage:?}"
    );
    assert!(
        damage.height() < VIEWPORT.y * 0.5,
        "the background painted under the whole window did not change, \
         so the menu appearing over it must not damage all of it: {damage:?}"
    );
}
