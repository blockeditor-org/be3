use super::*;

fn drawn(editor: &BeuiTest<TextApp>) -> Vec<(std::ops::Range<usize>, usize)> {
    let document = editor.document();
    let area = document
        .find_test_id("text.surface")
        .expect("the text is drawn");
    let surface = document
        .api::<beui::styled::TextAreaSurfaceApi>(area)
        .surface();
    document
        .api::<beui::styled::TextAreaApi>(surface)
        .remote_cursors()
        .into_iter()
        .map(|cursor| (cursor.selection, cursor.caret))
        .collect()
}

#[test]
fn peers_carets_follow_their_presence_and_go_when_it_is_hidden() {
    let mut editor = editor("hello brave new world");
    editor.presence_visible(true);
    editor.set_peers(None, vec![peer(7, 6, 11, PresenceColor::Purple)]);
    editor.run();
    assert_eq!(drawn(&editor), vec![(6..11, 11)]);

    editor.set_peers(None, vec![peer(7, 12, 12, PresenceColor::Purple)]);
    editor.run();
    assert_eq!(
        drawn(&editor),
        vec![(12..12, 12)],
        "a peer's caret moves when its presence does, with nothing else changing here"
    );

    editor.presence_visible(false);
    editor.run();
    assert!(drawn(&editor).is_empty(), "hidden presence draws no peers");

    editor.presence_visible(true);
    editor.run();
    assert_eq!(drawn(&editor), vec![(12..12, 12)]);
}
