use super::*;
use crate::reactive::*;
use crate::styled::*;
use crate::{HostPlacement, WindowId, WindowPlacement};

#[test]
fn closing_the_on_screen_keyboard_repaints_the_window_it_was_in() {
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
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let field = harness.find("field");
    let at = harness.rect(field).center();
    press_in(&mut harness, window, at, true);
    press_in(&mut harness, window, at, false);
    harness.context().toggle_keyboard();
    harness.settle();
    assert!(
        harness.context().keyboard_rect().is_some(),
        "the keyboard opens in the field's window"
    );

    harness.context().close_keyboard();
    harness.settle();
    assert!(harness.context().keyboard_rect().is_none());
}
