use super::*;

#[test]
fn the_window_keys_are_listed_in_the_command_palette() {
    let mut fixture = Fixture::with_windows(&[1, 2]);
    fixture.test.key(Key::P, Modifiers { shift: true, ..Modifiers::CTRL });
    fixture.settle();
    fixture.test.type_text("window");
    fixture.settle();
    for (label, keys) in [
        ("Close window", "Alt+F4"),
        ("Snap window to the left half", "Super+Left"),
        ("Snap window to the right half", "Super+Right"),
        ("Maximize window", "Super+Up"),
        ("Hide window", "Super+Down"),
    ] {
        assert!(fixture.says(label), "the palette lists {label}");
        assert!(fixture.says(keys), "with its key, {keys}");
    }
    fixture.test.snapshot("the_window_keys_in_the_command_palette");
}
