use be_sdk::{PointerButton, TimedInput};
use beui::styled::*;

use super::*;

#[test]
fn a_press_heard_behind_held_input_says_its_frame_has_input_waiting() {
    let _root = RootScope::new();
    let value = create_signal(String::new());
    let mut surface = launched(
        &EditorHost::default(),
        Windowed::new(move |id: ViewRef| match id.view.is_main() {
            true => view! {
                <TextInput value />
            },
            false => view! {
                <Frame width=Length::FILL height=Length::FILL />
            },
        }),
    );
    let main = whole(80.0, 60.0);
    let preview = preview();
    one(&mut surface, &main);
    surface.input(&main, &TimedInput::from(Input::Focus(true)), None);

    // The pointer rests on the preview before the app frames it, which holds what follows.
    surface.input(
        &preview,
        &TimedInput::from(Input::PointerRested(pos2(10.0, 10.0))),
        None,
    );
    let at = pos2(20.0, 10.0);
    for pressed in [true, false] {
        surface.input(
            &main,
            &TimedInput::from(Input::PointerButton {
                pos: at,
                button: PointerButton::Primary,
                pressed,
            }),
            None,
        );
    }
    surface.input(&main, &TimedInput::from(Input::Text("a".to_owned())), None);
    let waiting = surface.frame(&[main.clone(), preview.clone()]);
    assert!(
        waiting.iter().all(|frame| frame.input_waiting),
        "the frame that opened the preview's window has not taken in the press yet, and says so"
    );

    let answered = surface.frame(&[main.clone(), preview.clone()]);
    assert!(
        answered.iter().all(|frame| !frame.input_waiting),
        "the next frame takes it in"
    );
    assert_eq!(
        value.get_untracked(),
        "a",
        "and the field it pressed has the keyboard and the text typed after the press"
    );
    assert_eq!(surface.regions().keyboard_view(), Some(main.id()));
}
