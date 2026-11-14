use be_sdk::TimedInput;
use beui::styled::*;

use super::*;

#[test]
fn a_pointer_resting_on_a_view_before_its_first_frame_hovers_that_views_window() {
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

    // The host announced the preview and the pointer rests on it before the app has framed
    // it, so before the frame that opens its window.
    surface.input(
        &preview,
        &TimedInput::from(Input::PointerRested(pos2(10.0, 10.0))),
        None,
    );
    for _ in 0..2 {
        surface.frame(&[main.clone(), preview.clone()]);
    }
    assert!(
        preview_hovered.get_untracked(),
        "the view the pointer rests on hovers what is under it once its window opens"
    );
    assert!(
        !main_hovered.get_untracked(),
        "and the pointer does not land in another view's window meanwhile"
    );
}
