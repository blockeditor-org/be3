use super::*;
use crate::primitives::Frame;
use crate::primitives::List;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn clicking_an_open_menu_buttons_button_again_closes_its_menu() {
    let _root = crate::reactive::RootScope::new();
    let button = NodeRef::new();
    let open = create_signal(false);
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
                        open
                        on_select={|_: Vec<usize>| {}}
                    />
                </List>
            </Frame>
        }
    });
    let button = button.get();
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let at = harness.center(button);
    harness.click(at);
    harness.frame(Vec::new());
    assert!(open.get_untracked(), "the first click opens the menu");

    let at = harness.center(button);
    harness.click(at);
    for _ in 0..4 {
        harness.frame(Vec::new());
    }
    assert!(
        !open.get_untracked(),
        "a second click on the button closes its menu rather than closing and opening it again"
    );
}
