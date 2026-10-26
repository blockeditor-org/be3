use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn a_driver_pastes_the_headless_clipboard_with_ctrl_v() {
    let document = build_node(|| {
        let text = create_signal(String::new());
        view! {
            <TextInput @test_id={"field"} value={text} />
        }
    });
    let mut driven = headless(crate::testing::Showing(document));
    driven
        .ask(&["app", "click", "#field"])
        .expect("the field is focused");
    driven
        .ask(&["screen", "clipboard", "pasted"])
        .expect("the clipboard is set");

    let changed = driven
        .ask(&["app", "key", "ctrl+v"])
        .expect("the shortcut is pressed");
    assert!(
        changed.contains("pasted"),
        "Ctrl+V asks the window for its clipboard, as it does in any window:\n{changed}"
    );
}
