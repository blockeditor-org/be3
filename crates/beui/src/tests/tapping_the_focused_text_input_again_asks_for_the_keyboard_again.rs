use super::*;
use crate::styled::*;

fn keyboard(output: &crate::FrameOutput) -> Option<Keyboard> {
    output
        .main()
        .window
        .requests
        .ime
        .as_ref()
        .map(|area| area.keyboard)
}

#[test]
fn tapping_the_focused_text_input_again_asks_for_the_keyboard_again() {
    let (document, [input]) = toolbar_of(|| {
        [view! {
            <TextInput value="" />
        }]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let at = harness.center(input);

    harness.touch(TouchPhase::Start, at);
    let tapped = harness.touch(TouchPhase::End, at);
    let Some(Keyboard::Shown { asks }) = keyboard(&tapped) else {
        panic!(
            "a tap that focuses a text input asks for the keyboard: {:?}",
            keyboard(&tapped)
        );
    };

    let settled = harness.frame(Vec::new());
    assert_eq!(
        settled.main().window,
        tapped.main().window,
        "nothing changes until the field is tapped again"
    );

    harness.touch(TouchPhase::Start, at);
    let again = harness.touch(TouchPhase::End, at);
    assert_eq!(
        keyboard(&again),
        Some(Keyboard::Shown {
            asks: asks.wrapping_add(1)
        }),
        "tapping the field again asks for the keyboard again, which the user may have closed"
    );

    harness.touch(TouchPhase::Start, at);
    harness.touch(TouchPhase::Move, at + vec2(0.0, 40.0));
    let dragged = harness.touch(TouchPhase::End, at + vec2(0.0, 40.0));
    assert_eq!(
        keyboard(&dragged),
        keyboard(&again),
        "a drag that starts on the field is no tap"
    );
}
