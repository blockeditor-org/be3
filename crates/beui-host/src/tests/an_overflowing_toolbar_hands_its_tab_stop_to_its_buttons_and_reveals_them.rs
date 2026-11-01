use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::Scroll;
use beui::Direction;

#[test]
fn an_overflowing_toolbar_hands_its_tab_stop_to_its_buttons_and_reveals_them() {
    let _root = crate::reactive::RootScope::new();
    let (scroll, row) = (NodeRef::new(), NodeRef::new());
    let buttons: Vec<NodeRef> = (0..10).map(|_| NodeRef::new()).collect();
    let document = build_node({
        let buttons = buttons.clone();
        move || {
            arrow_focus(row, Direction::Horizontal);
            view! {
                <Scroll @node_ref=&scroll direction=Direction::Horizontal>
                    <List @node_ref=&row direction=Direction::Horizontal spacing=0.0>
                        <ForEach keys={indices(buttons.len())}>|index: usize| {
                            let button = buttons[index];
                            view! {
                                <Frame width=100.0>
                                    <LabelledButton
                                        @node_ref=&button
                                        label={format!("Button {index}")}
                                        on_click={move || {}}
                                    />
                                </Frame>
                            }
                        }</ForEach>
                    </List>
                </Scroll>
            }
        }
    });
    let scroll = scroll.get();
    let buttons: Vec<NodeId> = buttons.iter().map(NodeRef::get).collect();
    let mut harness = Harness::sized(document, Vec2::new(300.0, 100.0));
    harness.frame(Vec::new());
    let visible = |harness: &Harness, button: NodeId| {
        let rect = harness.rect(button);
        rect.left() >= 0.0 && rect.right() <= 300.0
    };

    harness.key(Key::Tab, Modifiers::NONE);
    assert!(
        harness
            .document()
            .api::<unstyled::ButtonApi>(buttons[0])
            .focused()
            .get()
    );
    for _ in 0..4 {
        harness.key(Key::Tab, Modifiers::NONE);
    }
    harness.frame(Vec::new());
    assert!(
        harness
            .document()
            .api::<unstyled::ButtonApi>(buttons[4])
            .focused()
            .get()
    );
    assert!(harness.document().scroll_offset(scroll) > 0.0);
    assert!(visible(&harness, buttons[4]));

    for _ in 0..4 {
        harness.key(Key::ArrowRight, Modifiers::NONE);
    }
    harness.frame(Vec::new());
    assert!(
        harness
            .document()
            .api::<unstyled::ButtonApi>(buttons[8])
            .focused()
            .get()
    );
    assert!(visible(&harness, buttons[8]));

    harness.key(Key::Home, Modifiers::NONE);
    harness.frame(Vec::new());
    assert!(
        harness
            .document()
            .api::<unstyled::ButtonApi>(buttons[0])
            .focused()
            .get()
    );
    assert_eq!(harness.document().scroll_offset(scroll), 0.0);

    harness.key(Key::End, Modifiers::NONE);
    harness.key(Key::ArrowLeft, Modifiers::NONE);
    harness.frame(Vec::new());
    assert!(
        harness
            .document()
            .api::<unstyled::ButtonApi>(buttons[8])
            .focused()
            .get()
    );
    assert!(visible(&harness, buttons[8]));
}
