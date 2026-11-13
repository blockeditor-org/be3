use super::*;

#[test]
fn a_hit_on_a_word_selects_it_whole_and_never_inside_another() {
    let text = "Chocolate keeps. Pears ripen late.";
    let late = text.rfind("late").unwrap();
    assert_eq!(
        crate::root::surface::find_words(text.as_bytes(), "late"),
        Some(late..late + 4),
        "\"late\" is the word, not the end of \"chocolate\""
    );
    assert_eq!(
        crate::root::surface::find_words(text.as_bytes(), "RIPEN late"),
        Some(text.find("ripen").unwrap()..late + 4),
        "words are matched as the index folds them, case and all"
    );
    assert_eq!(
        crate::root::surface::find_words("Café au lait".as_bytes(), "CAFÉ"),
        Some(0.."Café".len()),
        "case folding covers more than ASCII"
    );
    assert_eq!(
        crate::root::surface::find_words(text.as_bytes(), "ate"),
        None
    );

    let mut editor = editor(text);
    editor.presence_visible(true);
    editor.open_at(be_sdk_beui::SearchLocation {
        key: String::new(),
        text: "late".to_owned(),
    });
    editor.run();
    let cursor = shown_cursor(&mut editor)
        .flatten()
        .expect("the selection is shown to peers");
    assert_eq!(
        cursor["anchor"],
        serde_json::json!({ "client": 0, "offset": late }),
        "{cursor}"
    );
}
