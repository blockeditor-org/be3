use super::*;

#[test]
fn superscript_changes_the_next_key_only() {
    let mut typist = Typist::new();
    typist.swipe(Value::Char('s'), vec2(14.0, 14.0));
    assert_eq!(typist.engine.latch(Modifier::Superscript), Latch::Latched);
    assert_eq!(
        modify(Value::Char('a'), typist.engine.mods()),
        Value::Char('ᵃ'),
        "the labels show the superscript letters"
    );
    typist.swipe(Value::Char('w'), vec2(14.0, -14.0));
    typist.swipe(Value::Char('w'), vec2(14.0, -14.0));
    assert_eq!(typist.typed(), "²2");
}
