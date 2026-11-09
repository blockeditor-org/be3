use super::*;

#[test]
fn an_empty_text_given_the_keyboard_takes_the_caret() {
    let mut empty = editor("");
    empty.type_text("Hi");
    empty.run();
    assert_eq!(
        text(&empty),
        "Hi",
        "keys typed right after an empty text took the keyboard reach the text"
    );

    let mut kept = editor("Kept");
    kept.type_text("x");
    kept.run();
    assert_eq!(
        text(&kept),
        "Kept",
        "a text with words in it leaves the caret to the person"
    );
}
