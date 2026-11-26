use super::*;

#[test]
fn a_chord_handed_to_the_focus_keeps_its_place_among_what_follows() {
    let mut keys = intercepting();
    // The palette, a surface of the root's own, has the keyboard: the chord goes to it.
    let focus = Some(Handed::Focus(PALETTE, PALETTE_SURFACE));
    let x = Input::Text("x".to_owned());
    let heard = keys.hand(
        None,
        ROOT,
        focus,
        vec![
            to(ROOT, Surface::Root(0), tab(true), CTRL),
            to(PALETTE, PALETTE_SURFACE, x.clone(), CTRL),
        ],
        None,
    );
    let heard: Vec<(LayerId, &Input)> = heard
        .iter()
        .map(|delivery| (delivery.target, &delivery.input))
        .collect();
    assert_eq!(heard, [(PALETTE, &tab(true)), (PALETTE, &x)]);
    // Its release follows it there.
    let heard = keys.hand(
        None,
        ROOT,
        Some(Handed::Screen(SESSION, SESSION_SURFACE)),
        vec![to(ROOT, Surface::Root(0), tab(false), CTRL)],
        None,
    );
    assert_eq!(heard.len(), 1);
    assert_eq!((heard[0].target, &heard[0].input), (PALETTE, &tab(false)));
}
