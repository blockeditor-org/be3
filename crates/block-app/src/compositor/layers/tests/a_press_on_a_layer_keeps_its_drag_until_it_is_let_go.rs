use super::*;

#[test]
fn a_press_on_a_layer_keeps_its_drag_until_it_is_let_go() {
    let left = Rect::from_min_size(pos2(0.0, 0.0), vec2(100.0, 100.0));
    let right = Rect::from_min_size(pos2(200.0, 0.0), vec2(100.0, 100.0));
    let layers = vec![shown(1, 1, 1, left), shown(1, 2, 2, right)];
    let held = Cell::new(None);

    let runs = route(
        &input(vec![
            mouse(PointerPhase::Down(PointerButton::Primary), 50.0, 50.0),
            mouse(PointerPhase::Move, 250.0, 50.0),
            mouse(PointerPhase::Up(PointerButton::Primary), 250.0, 50.0),
        ]),
        &layers,
        &held,
    );
    assert_eq!(
        targets(&runs),
        vec![(1, LayerId(1))],
        "the whole press goes to the layer it began on"
    );
    assert!(runs[0].2, "the run holds a press, so its owner takes focus");
    assert_eq!(runs[0].1.len(), 3);

    let runs = route(
        &input(vec![mouse(PointerPhase::Move, 250.0, 50.0)]),
        &layers,
        &held,
    );
    assert_eq!(
        targets(&runs),
        vec![(1, LayerId(2))],
        "once let go, the pointer reaches the layer it is over"
    );
    assert!(!runs[0].2);
}
