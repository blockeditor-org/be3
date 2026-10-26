use super::*;

#[test]
fn fn_turns_digits_into_function_keys() {
    let mut typist = Typist::new();
    typist.tap(Value::Modifier(Modifier::Fn));
    typist.swipe(Value::Char('q'), vec2(14.0, -14.0));
    typist.tap(Value::Modifier(Modifier::Fn));
    typist.tap(Value::Char('e'));
    assert_eq!(typist.typed(), "[F1]€");
}
