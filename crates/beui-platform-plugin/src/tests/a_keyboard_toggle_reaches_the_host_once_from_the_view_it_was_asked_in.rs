use block_editor_plugin::TimedInput;

use super::*;

#[test]
fn a_keyboard_toggle_reaches_the_host_once_from_the_view_it_was_asked_in() {
    let _root = RootScope::new();
    let asked = Rc::new(Cell::new(false));
    let asking = Rc::clone(&asked);
    let mut surface = launched(
        &EditorHost::default(),
        Windowed::empty().hooked(move |context| {
            if asking.replace(false) {
                context.toggle_keyboard();
            }
        }),
    );
    let frame = whole(40.0, 30.0);
    let mut preview = whole(20.0, 10.0);
    preview.view = block_editor_plugin::View::PREVIEW;
    let toggles = |surface: &mut Driver<ModuleRunner>| {
        surface
            .frame(&[frame.clone(), preview.clone()])
            .into_iter()
            .map(|shown| shown.window.keyboard_toggles)
            .collect::<Vec<_>>()
    };
    assert_eq!(toggles(&mut surface), [0, 0]);

    surface.input(
        &frame,
        &TimedInput::from(Input::PointerMoved(pos2(10.0, 10.0))),
        None,
    );
    asked.set(true);
    assert_eq!(
        toggles(&mut surface),
        [1, 0],
        "the view the keyboard was toggled from tells the host"
    );

    surface.input(
        &preview,
        &TimedInput::from(Input::PointerMoved(pos2(5.0, 5.0))),
        None,
    );
    assert_eq!(
        toggles(&mut surface),
        [1, 0],
        "another view the input moves to does not toggle it again"
    );

    asked.set(true);
    assert_eq!(
        toggles(&mut surface),
        [1, 1],
        "a toggle asked from there is that view's own"
    );
}
