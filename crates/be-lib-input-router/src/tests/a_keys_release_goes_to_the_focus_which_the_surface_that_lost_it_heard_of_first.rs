use super::*;

/// As beui's forwarding catchers have it: the surface that lost the focus heard so first, which
/// lets go of the keys it held, so the release goes where the keys go now.
#[test]
fn a_keys_release_goes_to_the_focus_which_the_surface_that_lost_it_heard_of_first() {
    let test = canvas_with_editor();
    let mut router = Router::new(ROOT);
    router.focus(Some(EDITOR));
    assert_eq!(
        targets(&router.route(&test, key(Key::A, true))),
        vec![EDITOR]
    );
    assert_eq!(
        router.focus(Some(CANVAS)),
        vec![
            Routed::Input {
                target: EDITOR,
                input: Input::Focus(false),
            },
            Routed::Input {
                target: CANVAS,
                input: Input::Focus(true),
            },
        ]
    );
    assert_eq!(
        targets(&router.route(&test, key(Key::A, false))),
        vec![CANVAS]
    );
}
