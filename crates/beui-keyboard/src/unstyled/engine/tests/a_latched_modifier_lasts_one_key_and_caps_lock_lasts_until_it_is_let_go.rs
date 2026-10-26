use super::*;

#[test]
fn a_latched_modifier_lasts_one_key_and_caps_lock_lasts_until_it_is_let_go() {
    let mut typist = Typist::new();
    let shift = Value::Modifier(Modifier::Shift);
    typist.tap(shift);
    assert_eq!(typist.engine.latch(Modifier::Shift), Latch::Latched);
    typist.tap(Value::Char('a'));
    typist.tap(Value::Char('a'));
    assert_eq!(typist.typed(), "Aa", "shift lasts one key");

    typist.tap(shift);
    typist.swipe(Value::Char('e'), vec2(14.0, -14.0));
    assert_eq!(typist.typed(), "3", "shift leaves a digit a digit");

    typist.swipe(shift, vec2(14.0, -14.0));
    assert_eq!(typist.engine.latch(Modifier::Shift), Latch::Locked);
    typist.tap(Value::Char('a'));
    typist.tap(Value::Char('b'));
    assert_eq!(typist.typed(), "AB", "caps lock stays");
    typist.tap(shift);
    typist.tap(Value::Char('a'));
    assert_eq!(typist.typed(), "a", "tapping shift lets caps lock go");
}
