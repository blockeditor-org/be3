use be_sdk::{PointerButton, TimedInput};
use beui::styled::*;

use super::*;

#[test]
fn a_release_aimed_at_a_new_view_that_closes_ends_the_press_where_it_began() {
    let _root = RootScope::new();
    let clicks = create_signal(0);
    let mut surface = launched(
        &EditorHost::default(),
        Windowed::new(move |id: ViewRef| {
            let counts = id.view == be_sdk::View::PREVIEW.id;
            let clicked = move || {
                if counts {
                    clicks.update(|clicks| *clicks += 1);
                }
            };
            view! {
                <Interactive on_click={clicked}>
                    <Frame width=Length::FILL height=Length::FILL />
                </Interactive>
            }
        }),
    );
    let main = whole(80.0, 60.0);
    let pressed = preview();
    let mut new = preview();
    new.view = be_sdk::View::ARTIFACT_SETTINGS;
    surface.frame(&[main.clone(), pressed.clone()]);

    let at = pos2(10.0, 10.0);
    surface.input(
        &pressed,
        &TimedInput::from(Input::PointerButton {
            pos: at,
            button: PointerButton::Primary,
            pressed: true,
        }),
        None,
    );
    // The host aims the release at a view it has just announced, which closes before the app
    // ever frames it.
    surface.input(
        &pressed,
        &TimedInput::from(Input::PointerButton {
            pos: at,
            button: PointerButton::Primary,
            pressed: false,
        }),
        Some(new.id()),
    );
    for _ in 0..2 {
        surface.frame(&[main.clone(), pressed.clone()]);
    }
    assert_eq!(
        clicks.get_untracked(),
        1,
        "the release reaches the view it was pressed in, so the press ends as a click"
    );
}
