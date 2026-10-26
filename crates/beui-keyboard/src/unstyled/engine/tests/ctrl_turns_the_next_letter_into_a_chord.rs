use super::*;

#[test]
fn ctrl_turns_the_next_letter_into_a_chord() {
    let mut typist = Typist::new();
    typist.tap(Value::Modifier(Modifier::Ctrl));
    typist.tap(Value::Char('c'));
    typist.tap(Value::Char('c'));
    assert_eq!(typist.typed(), "[ctrl+C]c");

    let ctrl = typist.center(Value::Modifier(Modifier::Ctrl));
    let held = typist.down(ctrl);
    typist.tap(Value::Char('z'));
    typist.tap(Value::Char('z'));
    typist.engine.released(held, typist.now);
    typist.tap(Value::Char('z'));
    assert_eq!(
        typist.typed(),
        "[ctrl+Z][ctrl+Z]z",
        "a held modifier applies to every key while it is held and is not latched after"
    );
}
