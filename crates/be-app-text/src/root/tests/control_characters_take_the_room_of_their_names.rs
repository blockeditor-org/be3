use super::*;

#[test]
fn control_characters_take_the_room_of_their_names() {
    let mut editor =
        editor("nul\u{0}bel\u{7}del\u{7f}esc\u{1b}end\r\ntab\tspace zero\u{200b}width\r\nplain\n");
    editor.run();

    editor.snapshot("control_characters_take_the_room_of_their_names");
}
