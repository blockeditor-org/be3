use super::*;
use crate::keyboard::styled::key_center;
use crate::keyboard::unstyled::{Layer, Value};
use crate::reactive::*;
use crate::styled::*;

#[test]
fn a_key_let_go_over_the_inspector_is_not_left_held() {
    let _root = crate::reactive::RootScope::new();
    let typed = create_signal(String::new());
    let document = build_node(move || {
        view! {
            <TextInput @test_id="field" value={typed} />
        }
    });
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.context().enable_keyboard();
    harness.toggle_inspector();
    harness.frame(Vec::new());
    let field = harness.find("field");
    harness.click(harness.center(field));
    harness.context().toggle_keyboard();
    harness.frame(Vec::new());
    let keyboard = harness
        .context()
        .keyboard_rect()
        .expect("the keyboard is open");
    let panel = harness.inspector_panel_rect();

    let key = key_center(keyboard, Layer::Text, Value::Char('w')).expect("a w key");
    harness.drag(key, panel.center());
    harness.advance(Duration::from_secs(2));
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    assert_eq!(
        typed.get_untracked(),
        "2",
        "a drag up and right from w let go over the inspector types its corner once and is not left held to repeat"
    );
}
