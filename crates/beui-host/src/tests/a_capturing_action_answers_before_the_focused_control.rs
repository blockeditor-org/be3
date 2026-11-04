use super::*;
use crate::reactive::*;

#[test]
fn a_capturing_action_answers_before_the_focused_control() {
    let _root = crate::reactive::RootScope::new();
    let heard = create_signal(Vec::new());
    let (control, scoped) = (NodeRef::new(), NodeRef::new());
    let document = build_node(move || {
        Action::new("test.root.capture", "Root capture", move || {
            heard.update(|heard| heard.push("root capture"))
        })
        .shortcut(Chord::ctrl(Key::J))
        .capture()
        .register();
        Action::new("test.root.bubble", "Root bubble", move || {
            heard.update(|heard| heard.push("root bubble"))
        })
        .shortcut(Chord::ctrl(Key::K))
        .register();
        action_scope(&scoped);
        Action::new("test.scope.capture", "Scope capture", move || {
            heard.update(|heard| heard.push("scope capture"))
        })
        .shortcut(Chord::ctrl(Key::L))
        .capture()
        .register();
        view! {
            <List @node_ref=&scoped spacing=0.0>
                <unstyled::Button
                    @node_ref=&control
                    on_key={move |press: KeyPress| {
                        let taken = press.pressed && press.modifiers.ctrl;
                        if taken {
                            heard.update(|heard| heard.push("control"));
                        }
                        taken
                    }}
                >
                    <ButtonFace label="Control" />
                </unstyled::Button>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    harness.click(harness.center(control.get()));

    harness.key(Key::J, Modifiers::CTRL);
    harness.key(Key::K, Modifiers::CTRL);
    harness.key(Key::L, Modifiers::CTRL);
    assert_eq!(
        heard.get_untracked(),
        ["root capture", "control", "scope capture"],
        "a capturing action at the root or in a scope answers before the focused control, and a bubbling one only after it"
    );
}
