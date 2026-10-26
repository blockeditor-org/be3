use super::*;
use crate::keyboard::styled::key_center;
use crate::keyboard::unstyled::{Layer, Value};
use crate::reactive::*;
use crate::styled::*;

#[test]
fn a_key_dragged_off_the_software_keyboard_types_once_when_let_go() {
    let _root = crate::reactive::RootScope::new();
    let typed = create_signal(String::new());
    let document = build_node(move || {
        view! {
            <TextInput @test_id="field" value={typed} />
        }
    });
    let mut harness = Harness::new(document);
    harness.context().enable_keyboard();
    harness.frame(Vec::new());
    let field = harness.find("field");
    harness.click(harness.center(field));
    harness.context().toggle_keyboard();
    harness.frame(Vec::new());
    let keyboard = harness
        .context()
        .keyboard_rect()
        .expect("the keyboard is open");

    let key = key_center(keyboard, Layer::Text, Value::Char('w')).expect("a w key");
    let outside = pos2(key.x - 30.0, keyboard.top() - 40.0);
    harness.drag(key, outside);
    harness.frame(Vec::new());
    assert_eq!(
        typed.get_untracked(),
        "~",
        "the key's corner types when let go off the keyboard"
    );

    harness.advance(Duration::from_secs(2));
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    assert_eq!(typed.get_untracked(), "~", "nothing is left held to repeat");
}
