use super::*;
use crate::primitives::List;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn a_driver_holds_a_press_across_a_pause_and_taps_with_a_finger() {
    let _root = crate::reactive::RootScope::new();
    let clicks = create_signal(0);
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Button
                    label="Press"
                    variant=ButtonVariant::Primary
                    on_click={move || clicks.update(|clicks| *clicks += 1)}
                />
            </List>
        }
    });
    let mut driven = stepped(crate::testing::Showing(document));

    driven
        .ask(&["app", "down", "\"Press\""])
        .expect("the button is pressed");
    driven.ask(&["pause", "200"]).expect("time passes");
    assert_eq!(
        clicks.get_untracked(),
        0,
        "a held press has not clicked yet"
    );
    let state = driven.ask(&["app", "state"]).expect("the state is read");
    assert!(state.contains("holding Primary"), "{state}");
    driven.ask(&["app", "up"]).expect("the button is let go");
    assert_eq!(clicks.get_untracked(), 1, "letting go clicks");

    driven
        .ask(&["app", "tap", "\"Press\""])
        .expect("a finger taps the button");
    assert_eq!(clicks.get_untracked(), 2, "a tap clicks too");
}
