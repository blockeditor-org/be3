use super::*;

#[test]
fn a_peers_caret_and_selection_are_drawn_in_their_color() {
    let mut editor = editor("hello brave new world\nsecond line");
    editor.presence_visible(true);
    editor.set_peers(
        None,
        vec![
            peer(7, 6, 11, PresenceColor::Purple),
            peer(9, 29, 29, PresenceColor::Green),
        ],
    );
    editor.run();

    editor.snapshot("a_peers_caret_and_selection_are_drawn_in_their_color");

    editor.set_peers(None, vec![peer(9, 29, 29, PresenceColor::Green)]);
    editor.run();
    editor.snapshot("a_peers_caret_and_selection_are_drawn_in_their_color_after_one_leaves");
}
