use super::*;
use crate::KeyEvent;
use crate::primitives::*;
use crate::reactive::*;

#[component]
fn Holder(heard: Signal<Vec<bool>>) -> NodeId {
    let owned = store(Key::I);
    on_global_key(move |event: KeyEvent| {
        let key = owned.with(|key| *key);
        let Some(press) = event.press().filter(|press| press.key == key) else {
            return false;
        };
        heard.update(|heard| heard.push(press.pressed));
        true
    });
    view! {
        <Frame width=40.0 height=30.0 />
    }
}

#[test]
fn a_key_listener_gone_while_its_key_is_held_does_not_hear_the_release() {
    let _root = crate::reactive::RootScope::new();
    let shown = create_signal(true);
    let heard = create_signal(Vec::new());
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Show condition={shown}>
                    <Holder heard />
                </Show>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    harness.key_down(Key::I);
    shown.set(false);
    harness.frame(Vec::new());
    harness.key_up(Key::I);
    assert_eq!(
        heard.get_untracked(),
        vec![true],
        "the listener took the press, and the release found it gone with its component"
    );
}
