use super::*;
use crate::primitives::{Window, WindowHandle};
use crate::{WindowId, WindowReport};

#[test]
fn a_window_the_host_focuses_on_opening_gives_the_keyboard_to_its_first_focusable() {
    let _root = crate::reactive::RootScope::new();
    let window = WindowId::next();
    let main = create_signal(String::new());
    let typed = create_signal(String::new());
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <styled::TextInput @test_id="main" value={main} label="Main" />
                <Window id={window} size={vec2(300.0, 120.0)}>|_: WindowHandle|
                    <styled::TextInput @test_id="input" value={typed} label="Notes" />
                </Window>
            </List>
        }
    });
    let mut harness = Harness::hosting_windows(document, VIEWPORT);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let first = harness.find("main");
    let at = harness.rect(first).center();
    press_in(&mut harness, WindowId::MAIN, at, true);
    press_in(&mut harness, WindowId::MAIN, at, false);
    assert!(harness.document().focus_is_within(first));

    harness.report_window(WindowReport::Focused {
        window: WindowId::MAIN,
        focused: false,
    });
    harness.report_window(WindowReport::Focused {
        window,
        focused: true,
    });
    harness.frame(Vec::new());
    let input = harness.find("input");
    assert!(
        harness.document().focus_is_within(input),
        "a window the host focuses with nothing to give back focuses its first focusable"
    );
    harness.type_text("typed");
    harness.frame(Vec::new());
    assert_eq!(typed.get_untracked(), "typed");
    assert_eq!(main.get_untracked(), "");
}
