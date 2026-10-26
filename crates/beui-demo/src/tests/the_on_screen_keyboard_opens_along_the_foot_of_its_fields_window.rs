use beui::{HostPlacement, PointerButton, WindowId, WindowPlacement, vec2};

use super::*;

#[test]
fn the_on_screen_keyboard_opens_along_the_foot_of_its_fields_window() {
    let window = WindowId::next();
    let document = build(move || {
        let name = create_signal(String::new());
        view! {
            <Root>
                <Window
                    id={Some(window)}
                    size={vec2(480.0, 360.0)}
                    placement={WindowPlacement::Host(HostPlacement("a display".to_owned()))}
                >|_handle: WindowHandle|
                    <Frame padding={Padding::all(Space::Xl3)}>
                        <TextInput @test_id="field" value={name} label="Name" />
                    </Frame>
                </Window>
            </Root>
        }
    });
    let mut test = DocumentTest::hosting_windows(document, WIDE);
    test.context().enable_keyboard();
    test.context().set_keyboard_attached(false);
    test.frame(Vec::new());
    test.frame(Vec::new());
    let at = test.rect_of("field").center();
    for pressed in [true, false] {
        test.frame_in(
            window,
            vec![
                Input::PointerMoved(at),
                Input::PointerButton {
                    pos: at,
                    button: PointerButton::Primary,
                    pressed,
                },
            ],
        );
    }
    test.frame(Vec::new());
    let keyboard = test
        .context()
        .keyboard_rect()
        .expect("tapping the field opens the keyboard");
    assert_eq!(
        keyboard.bottom(),
        360.0,
        "along the foot of the field's window"
    );
    test.snapshot_window("on_screen_keyboard_in_its_fields_window", window);
}
