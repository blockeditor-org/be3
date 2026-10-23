use super::*;

#[test]
fn the_caret_is_shown_to_peers_in_a_color_they_are_not_using() {
    let mut editor = editor("hello world");
    editor.set_peers(None, vec![peer(7, 0, 0, PresenceColor::Red)]);
    editor.presence_visible(true);
    caret_at(&mut editor, 6);

    let cursor = shown_cursor(&mut editor)
        .flatten()
        .expect("the editor shows its caret to its peers");
    assert_eq!(
        cursor["focus"],
        serde_json::json!({ "client": 0, "offset": 6 }),
        "{cursor}"
    );
    assert_eq!(cursor["anchor"], cursor["focus"], "{cursor}");
    let color = cursor["color"].clone();
    assert_ne!(color, serde_json::json!(PresenceColor::Red));

    editor.key(
        Key::End,
        beui::Modifiers {
            shift: true,
            ..beui::Modifiers::NONE
        },
    );
    editor.run();
    let cursor = shown_cursor(&mut editor)
        .flatten()
        .expect("a new selection is shown to the peers");
    assert_eq!(
        cursor["anchor"],
        serde_json::json!({ "client": 0, "offset": 6 }),
        "{cursor}"
    );
    assert_eq!(
        cursor["focus"],
        serde_json::json!({ "client": 0, "offset": 11 }),
        "{cursor}"
    );
    assert_eq!(cursor["color"], color, "the color stays the same");

    editor.presence_visible(false);
    assert_eq!(
        shown_cursor(&mut editor),
        Some(None),
        "a hidden editor takes its caret back"
    );
}
