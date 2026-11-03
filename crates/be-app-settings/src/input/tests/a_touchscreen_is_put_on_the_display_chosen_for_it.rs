use super::*;

const DISPLAY: &str = "input.pointer.display";

#[test]
fn a_touchscreen_is_put_on_the_display_chosen_for_it() {
    let host = EditorHost::default();
    host.set_watched::<InputDevicesWatch>(&vec![mouse(), touchscreen()]);
    host.set_watched::<DisplaysWatch>(&vec![
        display("laptop", "Built-in display", "eDP-1"),
        display("WAC|Cintiq 16|1234", "Cintiq 16", "HDMI-A-1"),
    ]);
    host.set_watched::<InputReachWatch>(&InputReach {
        keyboard: true,
        middle_click_paste: true,
    });
    let mut editor = open(host);
    let chosen = |editor: &BeuiTest<InputSettingsRoot>| {
        content(editor)
            .root()
            .displays
            .get(&touchscreen_identity())
            .cloned()
    };

    choose_pointer(&mut editor, 1);
    assert!(
        !editor.shown(DISPLAY),
        "a mouse moves rather than lands, so it has no display"
    );

    choose_pointer(&mut editor, 2);
    editor.snapshot("a_touchscreen_is_put_on_the_display_chosen_for_it");
    assert_eq!(chosen(&editor), None, "it lands where it goes by itself");

    editor.click(DISPLAY);
    editor.run();
    editor.key(Key::Home, Modifiers::NONE);
    editor.run();
    editor.key(Key::ArrowDown, Modifiers::NONE);
    editor.run();
    editor.key(Key::ArrowDown, Modifiers::NONE);
    editor.run();
    editor.key(Key::Enter, Modifiers::NONE);
    editor.run();
    assert_eq!(chosen(&editor).as_deref(), Some("WAC|Cintiq 16|1234"));

    reset(&mut editor, DISPLAY);
    assert_eq!(chosen(&editor), None);
}
