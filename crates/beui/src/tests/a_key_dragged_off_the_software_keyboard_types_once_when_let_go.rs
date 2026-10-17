use super::*;
use crate::reactive::*;
use crate::styled::software_keyboard::key_center;
use crate::styled::*;
use crate::unstyled::software_keyboard::{Layer, Value};

#[test]
fn a_key_dragged_off_the_software_keyboard_types_once_when_let_go() {
    let typed = Rc::new(RefCell::new(String::new()));
    let seen = Rc::clone(&typed);
    let document = build(move || {
        view! {
            <TextInput
                @test_id="field"
                value=String::new()
                on_change={move |next: String| *seen.borrow_mut() = next}
            />
        }
    });
    let mut harness = Harness::new(document);
    harness.context().enable_keyboard();
    harness.frame(Vec::new());
    let field = harness.find("field");
    harness.click(harness.center(field));
    harness.context().toggle_keyboard();
    harness.frame(Vec::new());
    let keyboard = harness.context().keyboard_rect().expect("the keyboard is open");

    let key = key_center(keyboard, Layer::Text, Value::Char('w')).expect("a w key");
    let outside = pos2(key.x - 30.0, keyboard.top() - 40.0);
    harness.drag(key, outside);
    harness.frame(Vec::new());
    assert_eq!(*typed.borrow(), "~", "the key's corner types when let go off the keyboard");

    harness.advance(Duration::from_secs(2));
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    assert_eq!(*typed.borrow(), "~", "nothing is left held to repeat");
}
