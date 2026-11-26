use super::*;

#[test]
fn the_session_hears_only_the_modifiers_let_go_after_its_chord() {
    let mut keys = intercepting();
    let screen = Some(Handed::Screen(SESSION, SESSION_SURFACE));
    // Ctrl, then Shift, held before Tab: the router hands the modifiers to the focus.
    for held in [CTRL, CTRL_SHIFT] {
        let heard = keys.hand(None, ROOT, screen, Vec::new(), Some((held, None)));
        assert!(heard.is_empty(), "no chord is held yet: {heard:?}");
    }
    let heard = keys.hand(
        None,
        ROOT,
        screen,
        vec![to(ROOT, Surface::Root(0), tab(true), CTRL_SHIFT)],
        None,
    );
    let inputs: Vec<&Input> = heard.iter().map(|delivery| &delivery.input).collect();
    assert_eq!(
        inputs,
        [
            &Input::Modifiers(CTRL_SHIFT),
            &Input::InterceptedKey {
                key: Key::Tab,
                pressed: true,
                repeat: false,
            },
        ],
        "the session hears the chord with the modifiers held as it was pressed"
    );
    assert!(heard.iter().all(|delivery| delivery.target == SESSION));
    // Shift let go after the press: the session hears it, and Ctrl still held.
    let heard = keys.hand(None, ROOT, screen, Vec::new(), Some((CTRL, None)));
    let inputs: Vec<&Input> = heard.iter().map(|delivery| &delivery.input).collect();
    assert_eq!(inputs, [&Input::Modifiers(CTRL)]);
    let heard = keys.hand(
        None,
        ROOT,
        screen,
        Vec::new(),
        Some((Modifiers::NONE, None)),
    );
    let inputs: Vec<&Input> = heard.iter().map(|delivery| &delivery.input).collect();
    assert_eq!(inputs, [&Input::Modifiers(Modifiers::NONE)]);
    let heard = keys.hand(None, ROOT, screen, Vec::new(), Some((CTRL, None)));
    assert!(
        heard.is_empty(),
        "nothing is held of the chord any more: {heard:?}"
    );
}
