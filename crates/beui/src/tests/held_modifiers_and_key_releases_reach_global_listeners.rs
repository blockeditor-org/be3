use super::*;
use crate::KeyPress;
use crate::reactive::{build, create_effect, held_modifiers, on_global_key, view};
use crate::styled::TextInput;

#[test]
fn held_modifiers_and_key_releases_reach_global_listeners() {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let holds = Rc::new(RefCell::new(Vec::new()));
    let (keys, held) = (seen.clone(), holds.clone());
    let document = build(move || {
        on_global_key(move |press: KeyPress| {
            keys.borrow_mut().push((press.key, press.pressed));
            false
        });
        let modifiers = held_modifiers();
        create_effect(move || held.borrow_mut().push(modifiers.get()));
        view! {
            <List spacing=0.0>
                <TextInput @test_id={"field"} value="" />
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    harness.click(harness.center(harness.find("field")));

    harness.frame(vec![Event::Modifiers(Modifiers::ALT)]);
    harness.key(Key::Tab, Modifiers::ALT);
    harness.frame(vec![Event::Modifiers(Modifiers::NONE)]);

    assert_eq!(
        *seen.borrow(),
        vec![(Key::Tab, true), (Key::Tab, false)],
        "a listener sees the press and the release, though a text field has the focus"
    );
    assert_eq!(
        *holds.borrow(),
        vec![Modifiers::NONE, Modifiers::ALT, Modifiers::NONE],
        "the held modifiers follow the keyboard"
    );
}
