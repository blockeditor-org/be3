use super::*;
use crate::{Window, WindowHandle};
use crate::{WindowId, WindowReport};

#[test]
fn a_window_the_host_unfocuses_lets_go_of_the_keyboard_and_gets_it_back_when_refocused() {
    let _root = crate::reactive::RootScope::new();
    let window = WindowId::next();
    let typed = create_signal(String::new());
    let document = build_node(move || {
        view! {
            <Window id={window} size={vec2(300.0, 120.0)}>|_: WindowHandle|
                <styled::TextInput @test_id="input" value={typed} label="Notes" />
            </Window>
        }
    });
    let mut harness = Harness::hosting_windows(document, VIEWPORT);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let input = harness.find("input");
    let at = harness.rect(input).center();
    press_in(&mut harness, window, at, true);
    press_in(&mut harness, window, at, false);
    assert!(harness.document().focus_is_within(input));

    harness.report_window(WindowReport::Focused {
        window,
        focused: false,
    });
    harness.frame(Vec::new());
    assert!(
        !harness.document().focus_is_within(input),
        "a window the host unfocuses lets go of the keyboard"
    );
    harness.type_text("lost");
    harness.frame(Vec::new());
    assert_eq!(typed.get_untracked(), "");

    harness.report_window(WindowReport::Focused {
        window,
        focused: true,
    });
    harness.frame(Vec::new());
    assert!(
        harness.document().focus_is_within(input),
        "focused again, it gives the keyboard back to what had it"
    );
    harness.type_text("back");
    harness.frame(Vec::new());
    assert_eq!(typed.get_untracked(), "back");
}
