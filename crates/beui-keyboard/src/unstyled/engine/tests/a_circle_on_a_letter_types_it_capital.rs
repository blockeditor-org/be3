use super::*;

#[test]
fn a_circle_on_a_letter_types_it_capital() {
    let mut typist = Typist::new();
    typist.path(
        Value::Char('d'),
        &[
            vec2(0.0, -16.0),
            vec2(12.0, -14.0),
            vec2(16.0, 0.0),
            vec2(12.0, 14.0),
            vec2(0.0, 16.0),
        ],
    );
    typist.tap(Value::Char('d'));
    assert_eq!(typist.typed(), "Dd", "up, right and down makes a capital");
}
