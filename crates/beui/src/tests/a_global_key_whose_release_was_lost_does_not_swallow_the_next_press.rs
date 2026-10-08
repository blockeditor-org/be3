use super::*;
use crate::KeyPress;
use crate::reactive::{Action, Chord, build, on_shortcut, view};

#[test]
fn a_global_key_whose_release_was_lost_does_not_swallow_the_next_press() {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let document = build({
        let seen = Rc::clone(&seen);
        move || {
            Action::new("test.global", "Global", || {})
                .shortcut(Chord::logo(Key::G))
                .global()
                .register();
            on_shortcut(move |press: KeyPress| {
                seen.borrow_mut().push((press.key, press.pressed));
                false
            });
            view! {
                <List spacing=0.0>
                    <unstyled::Button>
                        <ButtonFace label="Button" />
                    </unstyled::Button>
                </List>
            }
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    harness.frame(vec![key_event(Key::G, true, Modifiers::LOGO)]);
    harness.frame(vec![Event::Focus(false)]);
    harness.key(Key::G, Modifiers::NONE);

    assert_eq!(
        *seen.borrow(),
        [(Key::G, true), (Key::G, false)],
        "the G after focus was lost reaches the document whole"
    );
}
