use super::*;

#[test]
fn compose_builds_a_letter_from_the_keys_after_it() {
    let mut typist = Typist::new();
    typist.tap(Value::Compose);
    assert!(typist.engine.composing());
    typist.swipe(Value::Char('m'), vec2(-14.0, 14.0));
    typist.tap(Value::Char('e'));
    assert!(!typist.engine.composing());
    typist.tap(Value::Char('e'));
    assert_eq!(typist.typed(), "ée");
}
