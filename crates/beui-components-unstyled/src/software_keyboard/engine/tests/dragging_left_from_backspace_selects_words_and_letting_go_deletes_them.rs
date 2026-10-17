use super::*;

#[test]
fn dragging_left_from_backspace_selects_words_and_letting_go_deletes_them() {
    let mut typist = Typist::new();
    let backspace = Value::Key(Key::Backspace);
    let moves: Vec<Vec2> = (1..=20).map(|step| vec2(-8.0 * step as f32, 0.0)).collect();
    typist.path(backspace, &moves);
    let typed = typist.typed();
    assert!(
        typed.starts_with("[ctrl+shift+ArrowLeft][ctrl+shift+ArrowLeft]"),
        "dragging selects whole words: {typed}"
    );
    assert!(
        typed.ends_with("[Backspace]"),
        "letting go deletes them: {typed}"
    );

    let mut back = moves.clone();
    back.extend(moves.iter().rev());
    typist.path(backspace, &back);
    let typed = typist.typed();
    assert!(
        typed.ends_with("[ctrl+shift+ArrowRight]"),
        "dragging back gives the words back and deletes nothing: {typed}"
    );

    typist.tap(backspace);
    assert_eq!(typist.typed(), "[Backspace]");
    typist.swipe(backspace, vec2(-14.0, 14.0));
    assert_eq!(
        typist.typed(),
        "[ctrl+Backspace]",
        "down left deletes a word"
    );
}
