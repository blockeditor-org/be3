use be_sdk::{Modifiers, TimedInput};

use super::*;

#[test]
fn a_key_released_through_a_view_that_closes_unframed_is_still_released() {
    let _root = RootScope::new();
    let mut surface = launched(&EditorHost::default(), Windowed::empty());
    let main = whole(80.0, 60.0);
    let pressed = preview();
    let mut new = preview();
    new.view = be_sdk::View::ARTIFACT_SETTINGS;
    surface.frame(&[main.clone(), pressed.clone()]);

    surface.input(&pressed, &TimedInput::from(Input::Focus(true)), None);
    let ctrl = Modifiers {
        ctrl: true,
        ..Modifiers::default()
    };
    surface.input(&pressed, &TimedInput::from(Input::Modifiers(ctrl)), None);
    surface.frame(&[main.clone(), pressed.clone()]);
    assert!(surface.regions().runner.modifiers().ctrl);

    // The keyboard moves to a view the host has just announced, and the key is let go there,
    // but the view closes before the app ever frames it.
    surface.input(&new, &TimedInput::from(Input::Focus(true)), None);
    surface.input(
        &new,
        &TimedInput::from(Input::Modifiers(Modifiers::default())),
        None,
    );
    for _ in 0..2 {
        surface.frame(&[main.clone(), pressed.clone()]);
    }
    assert!(
        !surface.regions().runner.modifiers().ctrl,
        "the key let go in a view that never showed is let go all the same"
    );
}
