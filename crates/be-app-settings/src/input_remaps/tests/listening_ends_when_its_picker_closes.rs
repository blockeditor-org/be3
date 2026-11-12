use super::*;
use be_sdk_beui::{CapturedChord, CapturedTrigger, InputCapture, InputCaptureWatch, WheelTurn};

#[test]
fn listening_ends_when_its_picker_closes() {
    let mut editor = editor(RemapState::On, Some(CAPS_TO_ESCAPE));

    editor.click("input-remaps.group.0.rule.0");
    editor.run();
    editor.click("input-remaps.group.0.rule.0.key");
    editor.run();
    editor.click("input-remaps.picker.listen");
    editor.run();
    editor.set_watched::<InputCaptureWatch>(&InputCapture {
        held: Vec::new(),
        chord: Some(CapturedChord {
            held: Vec::new(),
            trigger: CapturedTrigger::Wheel(WheelTurn::Up),
        }),
    });
    editor.run();
    assert!(
        editor.shown("input-remaps.picker.stop"),
        "a wheel turn is no key to send, so it listens on"
    );
    assert!(
        editor.watches::<InputCaptureWatch>(),
        "with a new capture"
    );
    assert_eq!(stored(&editor), CAPS_TO_ESCAPE);

    editor.key(Key::Escape, Modifiers::NONE);
    editor.run();
    assert!(!editor.shown("input-remaps.picker"));
    assert!(
        !editor.watches::<InputCaptureWatch>(),
        "listening ends with its picker"
    );

    editor.click("input-remaps.group.0.rule.0.on");
    editor.run();
    assert!(
        editor.shown("input-remaps.picker.listen"),
        "the next picker starts not listening"
    );
    assert!(!editor.watches::<InputCaptureWatch>());
    assert_eq!(
        stored(&editor),
        CAPS_TO_ESCAPE,
        "nothing caught for the other picker is picked here"
    );
}
