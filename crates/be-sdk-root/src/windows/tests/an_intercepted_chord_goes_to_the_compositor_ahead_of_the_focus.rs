use be_lib_ui_types::input::{Key, Keystroke, Modifiers};

use super::*;

fn escape(pressed: bool) -> Timed {
    Input::Key(Keystroke {
        key: Some(Key::Escape),
        pressed,
        ..Keystroke::default()
    })
    .into()
}

/// The layers each of `deliveries` is for, with whether it is a key.
fn targets(deliveries: &[Delivery]) -> Vec<(LayerId, bool)> {
    deliveries
        .iter()
        .map(|delivery| (delivery.target, matches!(delivery.input, Input::Key(_))))
        .collect()
}

#[test]
fn an_intercepted_chord_goes_to_the_compositor_ahead_of_the_focus() {
    let mut windows = Windows::new(ROOT);
    windows.drew(drawn());
    windows.route(
        None,
        vec![button(150.0, true).into(), button(150.0, false).into()],
    );
    assert_eq!(
        targets(&windows.route(None, vec![escape(true), escape(false)])),
        vec![(SCREEN, true), (SCREEN, true)],
        "before the root intercepts it, Escape goes to the focused screen"
    );

    windows.intercept(vec![KeyChord::new(Key::Escape, Modifiers::NONE)]);
    assert_eq!(
        targets(&windows.route(None, vec![escape(true), escape(false)])),
        vec![(ROOT, true), (ROOT, true)],
        "while the root intercepts it, Escape and its release go to the root's own layer"
    );

    windows.intercept(Vec::new());
    assert_eq!(
        targets(&windows.route(None, vec![escape(true)])),
        vec![(SCREEN, true)],
        "once the root lets go of it, Escape goes to the focus again"
    );
}
