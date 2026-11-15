use super::*;

/// A read-only block refuses edits and the text puts its content back, but a hex view that
/// took keys while read-only kept what they typed: the first digit of a byte waited for its
/// second, and completed a byte once the block could be edited again.
#[test]
fn a_read_only_text_in_hex_view_takes_no_typing_or_deleting() {
    let mut editor = editor("hello").with_top_bar(false);
    editor.pick_menu("text.hex");
    editor.run();
    editor.click("text.hex_bytes");

    editor.host().set_editable(false);
    editor.run();
    let applied = editor.store().applied(None);
    for (key, modifiers) in [
        (Key::Backspace, Modifiers::NONE),
        (Key::Delete, Modifiers::NONE),
        (Key::X, Modifiers::CTRL),
        (Key::Z, Modifiers::CTRL),
    ] {
        editor.key(key, modifiers);
    }
    editor.type_text("4");
    assert_eq!(text(&editor), "hello");
    assert_eq!(editor.store().applied(None), applied);

    editor.host().set_editable(true);
    editor.run();
    editor.type_text("1");
    assert_eq!(
        text(&editor),
        "hello",
        "one digit typed once the block is editable is half a byte"
    );
    editor.type_text("2");
    assert_eq!(text(&editor), "hello\x12");
}
