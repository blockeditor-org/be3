use super::*;

#[test]
fn a_layer_that_captures_presses_hands_those_outside_it_to_its_placer() {
    let mut popup = layer(3, pos2(150.0, 150.0), (40, 40));
    popup.input.captures = true;
    popup.input.focuses = false;
    popup.input.placer = Some(LayerId(1));
    let layers = window(vec![
        layer(1, pos2(0.0, 0.0), (400, 400)),
        layer(2, pos2(10.0, 10.0), (100, 100)),
        popup,
    ]);
    assert_eq!(layers.hit(pos2(20.0, 20.0)), Some(LayerId(1)));
    let mut router = Router::new(LayerId(1));
    let pressed = router.route(
        &layers,
        Input::PointerButton {
            pos: pos2(160.0, 160.0),
            button: PointerButton::Primary,
            pressed: true,
        },
    );
    assert_eq!(
        pressed,
        vec![Routed::Input {
            target: LayerId(3),
            input: Input::PointerButton {
                pos: pos2(20.0, 20.0),
                button: PointerButton::Primary,
                pressed: true,
            },
        }],
        "the popup takes its press in its own pixels, and not the focus"
    );
}
