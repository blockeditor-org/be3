use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn a_node_taking_clipboard_keys_tells_the_platform_while_the_focus_is_inside_it() {
    let _root = crate::reactive::RootScope::new();
    let heard = create_signal(Vec::<(Key, bool)>::new());
    let (inner, other) = (NodeRef::new(), NodeRef::new());
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Interactive clipboard_keys=true>
                    <Interactive
                        focusable=true
                        @node_ref=&inner
                        on_key={move |press: KeyPress| {
                            if press.pressed && press.modifiers.ctrl {
                                heard.update(|heard| heard.push((press.key, press.modifiers.shift)));
                            }
                            press.key == Key::C
                        }}
                    >
                        <Frame height=40.0 />
                    </Interactive>
                </Interactive>
                <Interactive focusable=true @node_ref=&other>
                    <Frame height=40.0 />
                </Interactive>
            </List>
        }
    });
    let (inner, other) = (inner.get(), other.get());
    let mut harness = Harness::new(document);
    let output = harness.frame(Vec::new());
    assert!(!output.main().window.requests.clipboard_keys);

    harness.click(harness.center(inner));
    let output = harness.frame(Vec::new());
    assert!(output.main().window.requests.clipboard_keys);

    let shifted = Modifiers {
        shift: true,
        ..Modifiers::CTRL
    };
    harness.key(Key::C, Modifiers::CTRL);
    harness.key(Key::C, shifted);
    harness.key(Key::V, shifted);
    harness.key(Key::A, Modifiers::CTRL);
    assert_eq!(
        heard.get_untracked(),
        [
            (Key::C, false),
            (Key::C, true),
            (Key::V, true),
            (Key::A, false)
        ],
        "Ctrl+Shift+C and Ctrl+Shift+V arrive as key presses"
    );
    assert!(
        !harness.inspector_open(),
        "Ctrl+Shift+C is the node's, not the inspector's picker"
    );

    harness.click(harness.center(other));
    let output = harness.frame(Vec::new());
    assert!(!output.main().window.requests.clipboard_keys);
}
