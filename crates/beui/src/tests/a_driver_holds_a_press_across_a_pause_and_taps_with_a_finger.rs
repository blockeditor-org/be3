use super::*;
use crate::reactive::{List, build, view};
use crate::styled::{Button, ButtonVariant};

#[test]
fn a_driver_holds_a_press_across_a_pause_and_taps_with_a_finger() {
    let clicks = Rc::new(Cell::new(0));
    let counted = clicks.clone();
    let document = build(move || {
        view! {
            <List spacing=0.0>
                <Button
                    label="Press"
                    variant=ButtonVariant::Primary
                    on_click={move || counted.set(counted.get() + 1)}
                />
            </List>
        }
    });
    let mut driven = Driven::new(document);

    driven
        .ask(&["down", "\"Press\""])
        .expect("the button is pressed");
    driven.ask(&["pause", "200"]).expect("time passes");
    assert_eq!(clicks.get(), 0, "a held press has not clicked yet");
    let state = driven.ask(&["state"]).expect("the state is read");
    assert!(state.contains("holding Primary"), "{state}");
    driven.ask(&["up"]).expect("the button is let go");
    assert_eq!(clicks.get(), 1, "letting go clicks");

    driven
        .ask(&["tap", "\"Press\""])
        .expect("a finger taps the button");
    assert_eq!(clicks.get(), 2, "a tap clicks too");
}
