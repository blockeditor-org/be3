use be_lib_input_router::{Routed, Router};
use be_lib_ui_types::input::{Input, PointerButton};

use super::*;

fn rect(x: f32, y: f32, width: f32, height: f32) -> Rect {
    Rect::from_min_max(pos2(x, y), pos2(x + width, y + height))
}

/// A surface of `size` pixels at two pixels per point, placed with its top left at `at`.
fn layer(id: u64, at: Pos2, size: (u32, u32)) -> Layer {
    Layer {
        id: LayerId(id),
        surface: Surface::Screen {
            instance: id as u32,
            surface: 0,
        },
        size,
        transform: Affine::from_axes(vec2(0.5, 0.0), vec2(0.0, 0.5), at),
        clip: Rect::from_min_max(Pos2::ZERO, pos2(size.0 as f32, size.1 as f32)),
        corner_radius: 0.0,
        opacity: 1.0,
        shown: true,
        input: LayerInput::default(),
    }
}

fn window(layers: Vec<Layer>) -> WindowLayers {
    WindowLayers {
        window: None,
        layers,
    }
}

fn close(one: Pos2, other: Pos2) -> bool {
    (one - other).length() < 1e-4
}

#[test]
fn a_transform_and_its_inverse_take_a_point_there_and_back() {
    let turned = Affine::from_axes(vec2(0.0, 2.0), vec2(-2.0, 0.0), pos2(30.0, 40.0));
    let point = pos2(5.0, 7.0);
    assert!(close(turned.apply(point), pos2(16.0, 50.0)));
    let back = turned.inverse().expect("a turn and a scale invert");
    assert!(close(back.apply(turned.apply(point)), point));
    let both = turned.then(&Affine::scaling(0.5));
    assert!(close(both.apply(point), pos2(8.0, 25.0)));
    assert_eq!(
        Affine::from_axes(vec2(1.0, 1.0), vec2(2.0, 2.0), Pos2::ZERO).inverse(),
        None,
        "a transform that flattens the surface has no inverse"
    );
}

#[test]
fn a_point_lands_on_the_topmost_layer_that_takes_it_there() {
    let mut top = layer(2, pos2(50.0, 50.0), (100, 100));
    top.clip = rect(0.0, 0.0, 50.0, 100.0);
    let layers = window(vec![layer(1, pos2(0.0, 0.0), (400, 400)), top]);
    assert_eq!(layers.hit(pos2(60.0, 60.0)), Some(LayerId(2)));
    assert_eq!(
        layers.hit(pos2(90.0, 60.0)),
        Some(LayerId(1)),
        "the part of a surface its clip hides takes nothing"
    );
    assert!(close(
        layers.local(LayerId(2), pos2(60.0, 60.0)).unwrap(),
        pos2(20.0, 20.0)
    ));
    let mut hidden = layers.clone();
    hidden.layers[1].shown = false;
    assert_eq!(hidden.hit(pos2(60.0, 60.0)), Some(LayerId(1)));
    let mut through = layers.clone();
    through.layers[1].input.hit = Area::Nowhere;
    assert_eq!(through.hit(pos2(60.0, 60.0)), Some(LayerId(1)));
}

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

#[test]
fn damage_is_each_surfaces_own_through_its_layer_and_the_bounds_of_layers_that_changed() {
    let before = window(vec![
        layer(1, pos2(0.0, 0.0), (200, 200)),
        layer(2, pos2(10.0, 10.0), (20, 20)),
        layer(3, pos2(50.0, 50.0), (20, 20)),
    ]);
    let mut after = before.clone();
    after.layers[1].transform.x = 20.0;
    after.layers.remove(2);
    after.layers.push(layer(4, pos2(80.0, 80.0), (20, 20)));
    let damage = before.damage(&after, |layer| {
        (layer.id == LayerId(1)).then(|| vec![rect(100.0, 100.0, 300.0, 4.0)])
    });
    assert_eq!(
        damage,
        vec![
            rect(10.0, 10.0, 10.0, 10.0),
            rect(20.0, 10.0, 10.0, 10.0),
            rect(80.0, 80.0, 10.0, 10.0),
            rect(50.0, 50.0, 10.0, 10.0),
            rect(50.0, 50.0, 50.0, 2.0),
        ],
        "the moved layer's old and new bounds, the new layer, the gone one, then the root's own \
         damage clipped to what it shows"
    );
}

#[test]
fn a_layer_raised_over_another_damages_both_of_its_places() {
    let before = window(vec![
        layer(1, pos2(0.0, 0.0), (20, 20)),
        layer(2, pos2(5.0, 5.0), (20, 20)),
    ]);
    let after = window(vec![before.layers[1].clone(), before.layers[0].clone()]);
    let damage = before.damage(&after, |_| Some(Vec::new()));
    assert!(
        damage.contains(&rect(0.0, 0.0, 10.0, 10.0)) && damage.contains(&rect(5.0, 5.0, 10.0, 10.0)),
        "{damage:?}"
    );
    assert!(
        before.damage(&before, |_| Some(Vec::new())).is_empty(),
        "nothing changed, nothing is damaged"
    );
}

#[test]
fn the_clip_rounds_only_the_surfaces_own_corners() {
    let mut cut = layer(1, Pos2::ZERO, (100, 100));
    assert_eq!(cut.rounded(), [true; 4]);
    cut.clip = rect(0.0, 0.0, 100.0, 60.0);
    assert_eq!(cut.rounded(), [true, true, false, false]);
}
