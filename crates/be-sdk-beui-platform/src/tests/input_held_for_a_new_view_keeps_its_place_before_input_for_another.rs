use be_sdk::TimedInput;
use beui::styled::*;

use super::*;

#[test]
fn input_held_for_a_new_view_keeps_its_place_before_input_for_another() {
    let _root = RootScope::new();
    let main_hovered = create_signal(false);
    let preview_hovered = create_signal(false);
    let mut surface = launched(
        &EditorHost::default(),
        Windowed::new(move |id: ViewRef| {
            let hovered = match id.view.is_main() {
                true => main_hovered,
                false => preview_hovered,
            };
            view! {
                <Interactive on_hover_change={hovered}>
                    <Frame width=Length::FILL height=Length::FILL />
                </Interactive>
            }
        }),
    );
    let main = whole(80.0, 60.0);
    let preview = preview();
    one(&mut surface, &main);

    // The pointer rests on the preview before the app frames it, then moves over the main view.
    surface.input(
        &preview,
        &TimedInput::from(Input::PointerRested(pos2(10.0, 10.0))),
        None,
    );
    surface.input(
        &main,
        &TimedInput::from(Input::PointerMoved(pos2(20.0, 20.0))),
        None,
    );
    for _ in 0..2 {
        surface.frame(&[main.clone(), preview.clone()]);
    }
    assert!(
        main_hovered.get_untracked(),
        "the pointer ends where it moved last"
    );
    assert!(
        !preview_hovered.get_untracked(),
        "the rest heard first does not take the pointer back to the new view"
    );
}
