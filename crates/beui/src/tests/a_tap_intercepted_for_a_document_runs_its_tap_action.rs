use super::*;
use crate::KeyChord;
use crate::reactive::{Action, Chord, build, view};

#[test]
fn a_tap_intercepted_for_a_document_runs_its_tap_action() {
    let runs = Rc::new(Cell::new(0));
    let counted = runs.clone();
    let document = build(move || {
        Action::new("test.launcher", "Programs", move || {
            counted.set(counted.get() + 1)
        })
        .shortcut(Chord::tap(Key::Logo))
        .intercepts()
        .register();
        view! {
            <List spacing=0.0 />
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    assert_eq!(
        harness.frame(Vec::new()).intercepted_keys,
        [KeyChord::tap(Key::Logo)],
        "an intercepting tap is named as a tap"
    );
    harness.frame(vec![Event::InterceptedTap(Key::Logo)]);
    assert_eq!(runs.get(), 1, "a tap intercepted for the document runs it");
    harness.frame(vec![Event::InterceptedTap(Key::Alt)]);
    assert_eq!(runs.get(), 1, "a tap of another key does not");
}
