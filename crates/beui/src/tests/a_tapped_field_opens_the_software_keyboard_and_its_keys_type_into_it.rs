use super::*;
use crate::native::keyboard::styled::key_center;
use crate::native::keyboard::unstyled::{Layer, Value};
use crate::reactive::*;
use crate::styled::*;

#[test]
fn a_tapped_field_opens_the_software_keyboard_and_its_keys_type_into_it() {
    let typed = Rc::new(RefCell::new(String::new()));
    let seen = Rc::clone(&typed);
    let document = build_node(move || {
        let name = create_signal(String::new());
        view! {
            <TextInput
                @test_id="field"
                value={Binding::new(name, move |next: String| {
                    *seen.borrow_mut() = next.clone();
                    name.set(next);
                })}
            />
        }
    });
    let mut harness = Harness::new(document);
    harness.context().enable_keyboard();
    harness.context().set_keyboard_attached(false);
    harness.frame(Vec::new());
    assert_eq!(
        harness.context().keyboard_rect(),
        None,
        "nothing asks for it yet"
    );

    let field = harness.find("field");
    harness.click(harness.center(field));
    harness.frame(Vec::new());
    let focused = harness.document().focused_node();
    let keyboard = harness
        .context()
        .keyboard_rect()
        .expect("tapping the field opens the keyboard");
    assert_eq!(keyboard.bottom(), VIEWPORT.y, "it sits along the bottom");

    for letter in ['h', 'i'] {
        let key = key_center(keyboard, Layer::Text, Value::Char(letter)).expect("a letter key");
        harness.click(key);
    }
    harness.settle();
    assert_eq!(
        *typed.borrow(),
        "hi",
        "its keys type into the focused field"
    );
    assert_eq!(
        harness.document().focused_node(),
        focused,
        "pressing keys leaves the focus in the field"
    );
}
