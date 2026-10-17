use super::*;

#[test]
fn a_held_key_repeats_until_it_is_let_go() {
    let mut typist = Typist::new();
    let backspace = typist.center(Value::Key(Key::Backspace));
    let held = typist.down(backspace);
    let due = typist
        .engine
        .tick(typist.now)
        .expect("a held key waits to repeat");
    assert_eq!(due, typist.now + LONG_PRESS);
    typist.now = due;
    typist.engine.tick(typist.now);
    typist.now += REPEAT;
    typist.engine.tick(typist.now);
    typist.engine.released(held, typist.now);
    assert_eq!(
        typist.typed(),
        "[Backspace][Backspace]",
        "letting go adds none"
    );

    let shift = typist.center(Value::Modifier(Modifier::Shift));
    let held = typist.down(shift);
    typist.now += LONG_PRESS;
    typist.engine.tick(typist.now);
    typist.engine.released(held, typist.now);
    assert_eq!(
        typist.engine.latch(Modifier::Shift),
        Latch::Locked,
        "a long press locks a modifier"
    );
}
