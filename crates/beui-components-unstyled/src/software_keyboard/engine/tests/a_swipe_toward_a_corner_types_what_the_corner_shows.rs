use super::*;

#[test]
fn a_swipe_toward_a_corner_types_what_the_corner_shows() {
    let mut typist = Typist::new();
    typist.swipe(Value::Char('q'), vec2(14.0, -14.0));
    typist.swipe(Value::Char('w'), vec2(-14.0, 14.0));
    typist.swipe(Value::Char('q'), vec2(14.0, 14.0));
    typist.tap(Value::Char('q'));
    assert_eq!(typist.typed(), "1@[Escape]q");
}
