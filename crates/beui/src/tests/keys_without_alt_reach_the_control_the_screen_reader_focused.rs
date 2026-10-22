use super::*;
use crate::reactive::*;
use crate::styled::Text;
use crate::styled::*;

#[test]
fn keys_without_alt_reach_the_control_the_screen_reader_focused() {
    let (document, [_heading, input]) = toolbar_of(|| {
        [
            view! {
                <Text style=TextStyle::HEADING>"Name"</Text>
            },
            view! {
                <TextInput value=String::new() />
            },
        ]
    });
    let mut harness = Harness::sized(document, TALL_VIEWPORT);
    harness.enable_screen_reader();
    harness.frame(Vec::new());

    harness.key(Key::ArrowRight, Modifiers::ALT);
    harness.frame(Vec::new());
    assert!(
        harness
            .document()
            .api::<unstyled::TextInputApi>(input)
            .focused()
            .get_untracked()
    );

    harness.type_text("hi");
    harness.key(Key::ArrowLeft, Modifiers::NONE);
    harness.type_text("!");
    harness.frame(Vec::new());

    assert_eq!(
        harness
            .document()
            .api::<styled::TextInputApi>(input)
            .value(),
        "h!i"
    );
}
