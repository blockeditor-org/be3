use super::*;
use crate::PasteRequest;
use crate::reactive::*;

#[test]
fn ctrl_v_taken_by_a_listener_asks_for_no_paste_unless_it_asks_itself() {
    let _root = crate::reactive::RootScope::new();
    let (asks, set_asks) = create_signal(false);
    let field = NodeRef::new();
    let document = build(move || {
        Action::new("test.paste", "Paste", move || {
            if asks.get_untracked() {
                request_paste();
            }
        })
        .shortcut(Chord::ctrl(Key::V))
        .register();
        view! {
            <List spacing=0.0>
                <unstyled::Button @node_ref=&field>
                    <ButtonFace label="Field" />
                </unstyled::Button>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    harness.click(harness.center(field.get()));
    let ctrl_v = || {
        Event::key(beui::KeyPress {
            key: Key::V,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::CTRL,
        })
    };

    let output = harness.frame(vec![ctrl_v()]);
    assert_eq!(
        output.paste, None,
        "an action took Ctrl+V for something else"
    );

    set_asks.set(true);
    let output = harness.frame(vec![ctrl_v()]);
    assert_eq!(
        output.paste,
        Some(PasteRequest::Shortcut),
        "an action that takes Ctrl+V to paste pastes as the shortcut does"
    );
}
