use super::*;

#[test]
fn a_text_opened_at_a_search_hit_selects_its_words() {
    let mut editor = editor("Orchard notes\nQuinces ripen late in the autumn");
    editor.presence_visible(true);
    editor.reveal(be_sdk_beui::SearchLocation {
        key: String::new(),
        text: "RIPEN".to_owned(),
    });
    editor.run();
    let cursor = shown_cursor(&mut editor)
        .flatten()
        .expect("the selection is shown to peers");
    assert_eq!(
        (cursor["anchor"].clone(), cursor["focus"].clone()),
        (
            serde_json::json!({ "client": 0, "offset": 22 }),
            serde_json::json!({ "client": 0, "offset": 27 }),
        ),
        "the words the search matched are selected, whatever their case: {cursor}"
    );
    editor.snapshot("a_text_opened_at_a_search_hit_selects_its_words");

    editor.reveal(be_sdk_beui::SearchLocation {
        key: "another document".to_owned(),
        text: "notes".to_owned(),
    });
    editor.run();
    assert!(
        shown_cursor(&mut editor).is_none(),
        "a place in a document the text does not have changes nothing"
    );
}
