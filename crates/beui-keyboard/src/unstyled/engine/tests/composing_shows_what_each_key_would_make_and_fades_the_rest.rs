use super::*;
use crate::unstyled::{Preview, preview};

#[test]
fn composing_shows_what_each_key_would_make_and_fades_the_rest() {
    let mut typist = Typist::new();
    typist.tap(Value::Compose);
    typist.tap(Value::Char('a'));
    let sequence = typist.engine.sequence().expect("still composing");
    assert_eq!(sequence, "a");
    assert_eq!(preview(sequence, Value::Char('e')), Preview::Becomes("æ"));
    assert_eq!(
        preview(sequence, Value::Char('a')),
        Preview::Becomes("å"),
        "a second a makes a ring"
    );
    assert_eq!(preview(sequence, Value::Char('q')), Preview::Faded);
    assert_eq!(preview(sequence, Value::Key(Key::Enter)), Preview::Faded);
    assert_eq!(preview(sequence, Value::Compose), Preview::Same);
    assert_eq!(
        preview("", Value::Char('\'')),
        Preview::Same,
        "a start of many sequences stays"
    );

    typist.tap(Value::Char('e'));
    assert_eq!(typist.typed(), "æ");
}
