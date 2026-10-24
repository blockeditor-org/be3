use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn the_software_keyboard_covers_the_share_of_the_screen_it_is_set_to() {
    let _root = crate::reactive::RootScope::new();
    let document = build_node(move || {
        view! {
            <TextInput @test_id="field" value=String::new() />
        }
    });
    let mut harness = Harness::sized(document, vec2(400.0, 800.0));
    harness.context().enable_keyboard();
    harness.frame(Vec::new());
    let field = harness.find("field");
    harness.click(harness.center(field));
    harness.context().toggle_keyboard();
    harness.frame(Vec::new());
    let height = |harness: &Harness| {
        harness
            .context()
            .keyboard_rect()
            .expect("the keyboard is open")
            .height()
    };
    assert_eq!(height(&harness), 800.0 * 0.35, "a default share");

    harness.context().set_keyboard_share(0.25);
    harness.frame(Vec::new());
    assert_eq!(height(&harness), 200.0);

    harness.context().set_keyboard_share(0.9);
    harness.frame(Vec::new());
    assert_eq!(height(&harness), 400.0, "never more than half the screen");
}
