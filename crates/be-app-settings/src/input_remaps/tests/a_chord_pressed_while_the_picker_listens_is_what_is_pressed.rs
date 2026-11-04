use super::*;
use be_sdk_beui::{CapturedChord, CapturedTrigger, InputCapture, InputCaptureWatch, WheelTurn};

#[test]
fn a_chord_pressed_while_the_picker_listens_is_what_is_pressed() {
    let mut editor = editor(RemapState::On, Some(CAPS_TO_ESCAPE));

    editor.click("input-remaps.group.0.rule.0");
    editor.run();
    editor.click("input-remaps.group.0.rule.0.on");
    editor.run();
    assert!(
        editor.shown("input-remaps.picker.listen"),
        "where the session filters input, the picker opens on Press it"
    );
    assert!(
        !editor.watches::<InputCaptureWatch>(),
        "nothing is caught until it listens"
    );
    editor.click("input-remaps.picker.listen");
    editor.run();
    assert!(editor.watches::<InputCaptureWatch>());
    editor.set_watched::<InputCaptureWatch>(&InputCapture {
        held: vec![29, 190],
        chord: None,
    });
    editor.run();
    editor.snapshot("the_key_picker_shows_what_is_held_while_it_listens");

    editor.set_watched::<InputCaptureWatch>(&InputCapture {
        held: Vec::new(),
        chord: Some(CapturedChord {
            held: vec![29, 190],
            trigger: CapturedTrigger::Wheel(WheelTurn::Up),
        }),
    });
    editor.run();
    assert!(!editor.shown("input-remaps.picker"));
    assert!(
        !editor.watches::<InputCaptureWatch>(),
        "the capture ends with the picker"
    );
    let text = stored(&editor);
    assert!(
        text.contains(r#"{ on = "Ctrl+F20+WheelUp", send = "Escape" }"#),
        "held modifiers and keys come with what is pressed: {text}"
    );

    editor.click("input-remaps.group.0.rule.0.key");
    editor.run();
    editor.click("input-remaps.picker.listen");
    editor.run();
    editor.set_watched::<InputCaptureWatch>(&InputCapture {
        held: Vec::new(),
        chord: Some(CapturedChord {
            held: vec![42, 190],
            trigger: CapturedTrigger::Source(17),
        }),
    });
    editor.run();
    let text = stored(&editor);
    assert!(
        text.contains(r#"send = "Shift+W""#),
        "a key sent keeps only the modifiers held with it: {text}"
    );
}
