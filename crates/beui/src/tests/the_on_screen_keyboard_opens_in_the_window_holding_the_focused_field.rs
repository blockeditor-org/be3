use super::*;
use crate::native::keyboard::styled::key_center;
use crate::native::keyboard::unstyled::{Layer as KeyLayer, Value};
use crate::reactive::*;
use crate::styled::*;
use crate::{HostPlacement, WindowId, WindowPlacement};

#[test]
fn the_on_screen_keyboard_opens_in_the_window_holding_the_focused_field() {
    let _root = crate::reactive::RootScope::new();
    let window = WindowId::next();
    let typed = create_signal(String::new());
    let document = build_node(move || {
        view! {
            <Window
                id={Some(window)}
                size={vec2(500.0, 400.0)}
                placement={WindowPlacement::Host(HostPlacement("a display".to_owned()))}
            >|_handle: WindowHandle|
                <styled::Frame padding={Padding::all(Space::Xl3)}>
                    <TextInput @test_id="field" value={typed} />
                </styled::Frame>
            </Window>
        }
    });
    let mut harness = Harness::hosting_windows(document, WIDE_VIEWPORT);
    harness.context().enable_keyboard();
    harness.context().set_keyboard_attached(false);
    harness.frame(Vec::new());
    harness.frame(Vec::new());

    let field = harness.find("field");
    let at = harness.rect(field).center();
    press_in(&mut harness, window, at, true);
    press_in(&mut harness, window, at, false);
    harness.frame(Vec::new());
    let keyboard = harness
        .context()
        .keyboard_rect()
        .expect("tapping the field opens the keyboard");
    assert_eq!(
        keyboard.bottom(),
        400.0,
        "it sits along the bottom of the field's window, not the main one"
    );
    assert_eq!(keyboard.width(), 500.0, "across that window");

    for letter in ['o', 'k'] {
        let key = key_center(keyboard, KeyLayer::Text, Value::Char(letter)).expect("a letter key");
        press_in(&mut harness, window, key, true);
        press_in(&mut harness, window, key, false);
    }
    harness.settle();
    assert_eq!(
        typed.get_untracked(),
        "ok",
        "its keys, pressed in that window, type into the field"
    );
}
